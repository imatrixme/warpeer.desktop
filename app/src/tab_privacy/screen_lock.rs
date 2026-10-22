use std::ptr::NonNull;

use block2::RcBlock;
use futures::channel::mpsc;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSNotificationName,
    NSObjectProtocol, NSString,
};
use warpui::{Entity, ModelContext, SingletonEntity};

use super::{TabPrivacyRuntime, TabPrivacySettings};

pub struct ScreenLockMonitor {
    center: Retained<NSDistributedNotificationCenter>,
    observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl ScreenLockMonitor {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let center = NSDistributedNotificationCenter::defaultCenter();
        let (sender, receiver) = mpsc::unbounded();
        let callback = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = sender.unbounded_send(());
        });
        let name = NSString::from_str("com.apple.screenIsLocked");
        let notification_center =
            unsafe { &*Retained::as_ptr(&center).cast::<NSNotificationCenter>() };
        let observer = unsafe {
            notification_center.addObserverForName_object_queue_usingBlock(
                Some(&name as &NSNotificationName),
                None::<&AnyObject>,
                None,
                &callback,
            )
        };
        ctx.spawn_stream_local(
            receiver,
            |_, (), ctx| {
                if *TabPrivacySettings::as_ref(ctx).lock_on_system_inactivity {
                    TabPrivacyRuntime::handle(ctx)
                        .update(ctx, |runtime, ctx| runtime.lock_all(ctx));
                }
            },
            |_, _| {},
        );
        Self { center, observer }
    }
}

impl Drop for ScreenLockMonitor {
    fn drop(&mut self) {
        unsafe {
            let center = &*Retained::as_ptr(&self.center).cast::<NSNotificationCenter>();
            let observer = &*Retained::as_ptr(&self.observer).cast::<AnyObject>();
            center.removeObserver(observer);
        }
    }
}

impl Entity for ScreenLockMonitor {
    type Event = ();
}

impl SingletonEntity for ScreenLockMonitor {}
