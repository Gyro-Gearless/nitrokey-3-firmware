use crate::types::usbnfc::UsbClasses;
use crate::types::*;

use core::sync::atomic::{AtomicBool, Ordering::Relaxed};

use apdu_dispatch::dispatch::Interface;
use embedded_time::duration::Milliseconds;

// I am pretty sure this does not belong here,
// anyways better than having this both UIs:
// dummy_ui.rs, trussed_ui.rs
// so how about a `base_ui.rs` ?
// -> also the whole RGB stuff and its "ecosystem" is widely hw-independant and could fit there....
static WAITING: AtomicBool = AtomicBool::new(false);
pub struct UserPresenceStatus {}
impl UserPresenceStatus {
    pub fn set_waiting(waiting: bool) {
        WAITING.store(waiting, Relaxed);
    }
    pub fn waiting() -> bool {
        WAITING.load(Relaxed)
    }
}

pub fn poll_dispatchers(
    apdu_dispatch: &mut ApduDispatch,
    ctaphid_dispatch: &mut CtaphidDispatch,
    apps: &mut Apps,
) -> (bool, bool) {
    let apdu_poll = apps.apdu_dispatch(|apps| apdu_dispatch.poll(apps));
    let ctaphid_poll = apps.ctaphid_dispatch(|apps| ctaphid_dispatch.poll(apps));

    (
        apdu_poll == Some(Interface::Contact) || ctaphid_poll,
        apdu_poll == Some(Interface::Contactless),
    )
}

/* ************************************************************************ */

pub fn poll_usb<S, FA, FB, TA, TB, E>(
    usb_classes: &mut Option<UsbClasses<S>>,
    ccid_spawner: FA,
    ctaphid_spawner: FB,
    t_now: Milliseconds,
) where
    S: Soc,
    FA: Fn(S::Duration) -> Result<TA, E>,
    FB: Fn(S::Duration) -> Result<TB, E>,
{
    let Some(usb_classes) = usb_classes.as_mut() else {
        return;
    };

    usb_classes.ctaphid.check_timeout(t_now.0);
    usb_classes.poll();

    maybe_spawn_ccid(usb_classes.ccid.did_start_processing(), ccid_spawner);
    maybe_spawn_ctaphid(usb_classes.ctaphid.did_start_processing(), ctaphid_spawner);
}

pub fn poll_nfc<D, F, T, E>(contactless: &mut Option<Iso14443>, nfc_spawner: F)
where
    D: From<Milliseconds>,
    F: Fn(D) -> Result<T, E>,
{
    let Some(contactless) = contactless.as_mut() else {
        return;
    };

    maybe_spawn_nfc(contactless.poll(), nfc_spawner);
}

/* ************************************************************************ */

pub fn ccid_keepalive<S, F, T, E>(usb_classes: &mut Option<UsbClasses<S>>, ccid_spawner: F)
where
    S: Soc,
    F: Fn(S::Duration) -> Result<T, E>,
{
    if usb_classes.is_none() {
        return;
    }

    let usb_classes = usb_classes.as_mut().unwrap();

    maybe_spawn_ccid(usb_classes.ccid.send_wait_extension(), ccid_spawner);
}

pub fn ctaphid_keepalive<S, F, T, E>(usb_classes: &mut Option<UsbClasses<S>>, ctaphid_spawner: F)
where
    S: Soc,
    F: Fn(S::Duration) -> Result<T, E>,
{
    if usb_classes.is_none() {
        return;
    }
    let usb_classes = usb_classes.as_mut().unwrap();

    maybe_spawn_ctaphid(
        usb_classes
            .ctaphid
            .send_keepalive(UserPresenceStatus::waiting()),
        ctaphid_spawner,
    );
}

pub fn nfc_keepalive<D, F, T, E>(contactless: &mut Option<Iso14443>, nfc_spawner: F)
where
    D: From<Milliseconds>,
    F: Fn(D) -> Result<T, E>,
{
    if contactless.is_none() {
        return;
    }

    let contactless = contactless.as_mut().unwrap();

    maybe_spawn_nfc(contactless.poll_wait_extensions(), nfc_spawner);
}

/* ************************************************************************ */

fn maybe_spawn_ccid<D, F, T, E>(status: usbd_ccid::Status, ccid_spawner: F)
where
    D: From<Milliseconds>,
    F: Fn(D) -> Result<T, E>,
{
    if let usbd_ccid::Status::ReceivedData(ms) = status {
        ccid_spawner(ms.into()).ok();
    };
}

fn maybe_spawn_ctaphid<D, F, T, E>(status: usbd_ctaphid::types::Status, ctaphid_spawner: F)
where
    D: From<Milliseconds>,
    F: Fn(D) -> Result<T, E>,
{
    if let usbd_ctaphid::types::Status::ReceivedData(ms) = status {
        ctaphid_spawner(ms.into()).ok();
    };
}

fn maybe_spawn_nfc<D, F, T, E>(status: nfc_device::Iso14443Status, nfc_spawner: F)
where
    D: From<Milliseconds>,
    F: Fn(D) -> Result<T, E>,
{
    if let nfc_device::Iso14443Status::ReceivedData(ms) = status {
        nfc_spawner(ms.into()).ok();
    };
}

/* ************************************************************************ */

pub fn run_trussed(trussed: &mut Trussed) {
    trussed.process();
}
