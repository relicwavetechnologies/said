use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyAccessory, NSMenu, NSMenuItem, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::{Arc, Mutex};

use crate::app::AppCore;
use crate::config;

static mut STATUS_ITEM: Option<id> = None;
static mut MODE_ITEM: Option<id> = None;
static mut APP_CORE_REF: Option<Arc<Mutex<AppCore>>> = None;

pub fn set_title(title: &str) {
    unsafe {
        if let Some(item) = STATUS_ITEM {
            let s = NSString::alloc(nil).init_str(title);
            let button: id = msg_send![item, button];
            let _: () = msg_send![button, setTitle: s];
        }
    }
}

pub fn reset_core_state() {
    unsafe {
        if let Some(ref core) = APP_CORE_REF {
            core.lock().unwrap().reset_to_idle();
        }
    }
}

fn update_mode_item() {
    unsafe {
        if let Some(item) = MODE_ITEM {
            let label = config::mode_label();
            let s = NSString::alloc(nil).init_str(label);
            let _: () = msg_send![item, setTitle: s];
        }
    }
}

extern "C" fn on_toggle(_this: &Object, _cmd: Sel, _sender: id) {
    unsafe {
        if let Some(ref core) = APP_CORE_REF {
            core.lock().unwrap().toggle();
        }
    }
}

extern "C" fn on_cycle_mode(_this: &Object, _cmd: Sel, _sender: id) {
    let mode = config::cycle_mode();
    println!("[app] mode → {}", mode.key);
    update_mode_item();
    set_title(mode.icon);
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(800));
        dispatch::Queue::main().exec_async(|| set_title("●"));
    });
}

extern "C" fn on_quit(_this: &Object, _cmd: Sel, _sender: id) {
    unsafe {
        let app = NSApp();
        let _: () = msg_send![app, terminate: nil];
    }
}

fn register_delegate_class() -> &'static Class {
    let superclass = class!(NSObject);
    let mut decl = ClassDecl::new("VPDelegate", superclass).unwrap();

    unsafe {
        decl.add_method(
            sel!(onToggle:),
            on_toggle as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(onCycleMode:),
            on_cycle_mode as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(onQuit:),
            on_quit as extern "C" fn(&Object, Sel, id),
        );
    }

    decl.register()
}

pub fn run(core: Arc<Mutex<AppCore>>) {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);

        APP_CORE_REF = Some(core);

        let delegate_class = register_delegate_class();
        let delegate: id = msg_send![delegate_class, new];

        // Status bar item
        let status_bar = NSStatusBar::systemStatusBar(nil);
        let item = status_bar.statusItemWithLength_(NSVariableStatusItemLength);
        let button: id = msg_send![item, button];
        let title = NSString::alloc(nil).init_str("●");
        let _: () = msg_send![button, setTitle: title];
        STATUS_ITEM = Some(item);

        // Menu
        let menu = NSMenu::new(nil).autorelease();

        // Toggle recording
        let toggle_title = NSString::alloc(nil).init_str("🎤  Toggle recording  (fn+Shift)");
        let toggle_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            toggle_title,
            sel!(onToggle:),
            NSString::alloc(nil).init_str(""),
        );
        let _: () = msg_send![toggle_item, setTarget: delegate];
        menu.addItem_(toggle_item);

        // Mode cycle
        let mode_title = NSString::alloc(nil).init_str(config::mode_label());
        let mode_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            mode_title,
            sel!(onCycleMode:),
            NSString::alloc(nil).init_str(""),
        );
        let _: () = msg_send![mode_item, setTarget: delegate];
        menu.addItem_(mode_item);
        MODE_ITEM = Some(mode_item);

        // Separator
        let sep: id = msg_send![class!(NSMenuItem), separatorItem];
        menu.addItem_(sep);

        // Quit
        let quit_title = NSString::alloc(nil).init_str("Quit Voice Polish");
        let quit_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            quit_title,
            sel!(onQuit:),
            NSString::alloc(nil).init_str("q"),
        );
        let _: () = msg_send![quit_item, setTarget: delegate];
        menu.addItem_(quit_item);

        item.setMenu_(menu);

        println!("[menubar] ● ready");
        app.run();
    }
}
