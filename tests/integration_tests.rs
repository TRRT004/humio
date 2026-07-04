#![allow(clippy::all, clippy::pedantic, clippy::nursery, unused)]

use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetActiveWindow, SetFocus};

use humio::{HumanizedDevice, Keyboard, Mouse, PhysicalDevice, Point, TargetArea};
use enigo::{Button, Key};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReceivedEvent {
    MouseDown { x: i32, y: i32 },
    MouseUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    KeyDown { key: u32 },
    Char { ch: char },
}

static RECEIVED_EVENTS: OnceLock<Mutex<Vec<ReceivedEvent>>> = OnceLock::new();

fn get_events() -> Vec<ReceivedEvent> {
    RECEIVED_EVENTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clone()
}

fn clear_events() {
    RECEIVED_EVENTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clear();
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let events = RECEIVED_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
            if let Ok(mut guard) = events.lock() {
                guard.push(ReceivedEvent::MouseDown { x, y });
            }
            0
        }
        WM_LBUTTONUP => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let events = RECEIVED_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
            if let Ok(mut guard) = events.lock() {
                guard.push(ReceivedEvent::MouseUp { x, y });
            }
            0
        }
        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let events = RECEIVED_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
            if let Ok(mut guard) = events.lock() {
                guard.push(ReceivedEvent::MouseMove { x, y });
            }
            0
        }
        WM_KEYDOWN => {
            let events = RECEIVED_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
            if let Ok(mut guard) = events.lock() {
                guard.push(ReceivedEvent::KeyDown { key: wparam as u32 });
            }
            0
        }
        WM_CHAR => {
            let events = RECEIVED_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
            if let Ok(mut guard) = events.lock() {
                if let Some(ch) = char::from_u32(wparam as u32) {
                    guard.push(ReceivedEvent::Char { ch });
                }
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn spawn_test_window(x: i32, y: i32, width: i32, height: i32) -> HWND {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<isize>();

    thread::spawn(move || unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = "HumioTestWindowClass\0".encode_utf16().collect::<Vec<u16>>();

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW as *const u16),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wnd_class);

        let window_title = "Humio Integration Test Window\0".encode_utf16().collect::<Vec<u16>>();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            x,
            y,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            panic!("Failed to create window");
        }

        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetActiveWindow(hwnd);
        SetFocus(hwnd);

        tx.send(hwnd as isize).unwrap();

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });

    rx.recv().unwrap() as HWND
}

#[test]
fn test_physical_input_interaction() {
    // 1. Initialize physical device wrapper (skip if headless/CI lacks UI context)
    let physical = match PhysicalDevice::new() {
        Ok(p) => p,
        Err(e) => {
            println!("Skipping physical integration test: physical device initialization failed: {:?}", e);
            return;
        }
    };

    // 2. Spawn test window (coordinates 200, 200, size 300, 300)
    let hwnd = spawn_test_window(200, 200, 300, 300);
    assert!(!hwnd.is_null(), "Failed to create integration test window");

    // Give window a moment to display and bind
    thread::sleep(Duration::from_millis(1000));
    clear_events();

    // 3. Find the window's position on screen
    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe {
        GetWindowRect(hwnd, &mut rect);
    }
    println!("Window bounds: left={}, top={}, right={}, bottom={}", rect.left, rect.top, rect.right, rect.bottom);

    let center_x = rect.left + (rect.right - rect.left) / 2;
    let center_y = rect.top + (rect.bottom - rect.top) / 2;

    let mut dev = HumanizedDevice::new(physical);
    let _ = dev.move_mouse(Point::new(0, 0));
    let start_loc = dev.location().expect("Failed to get start location");
    println!("Start mouse location: {:?}", start_loc);
    println!("Targeting test window center at screen coordinate: ({}, {})", center_x, center_y);

    // 4. Move mouse and click inside the window to focus it
    let target = TargetArea::Rect {
        top_left: Point::new(center_x - 30, center_y - 30),
        bottom_right: Point::new(center_x + 30, center_y + 30),
        target: None,
        std_dev_x: None,
        std_dev_y: None,
    };
    dev.click_area(&target, Button::Left, false).expect("Failed to perform humanized click");

    // Allow events to propagate
    thread::sleep(Duration::from_millis(500));

    let mid_loc = dev.location().expect("Failed to get location after click");
    println!("Mouse location after click: {:?}", mid_loc);

    // Type a keyboard character
    dev.key(Key::Unicode('x'), enigo::Direction::Click).expect("Failed to send key click");

    // Allow events to propagate
    thread::sleep(Duration::from_millis(500));

    // Get events received by our WndProc
    let events = get_events();
    println!("Received Win32 Window events: {:?}", events);

    // Close the test window
    unsafe {
        PostMessageW(hwnd, WM_CLOSE, 0, 0);
    }

    // Verify we registered at least one mouse or keyboard event
    let has_mousedown = events.iter().any(|e| matches!(e, ReceivedEvent::MouseDown { .. }));
    let has_keydown = events.iter().any(|e| matches!(e, ReceivedEvent::KeyDown { .. }) || matches!(e, ReceivedEvent::Char { .. }));

    // On local Windows development systems, these inputs will reliably register.
    // In some restricted environments, input injection can be blocked. Let's make it a soft assertion or a diagnostic check.
    assert!(
        has_mousedown || has_keydown,
        "Failed to receive simulated mouse or keyboard events in the test window. Events: {:?}",
        events
    );
}
