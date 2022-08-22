use anyhow::{anyhow, Result};
use detour::static_detour;
use imgui::Context;
use imgui_opengl_renderer::Renderer;
use std::{
    ffi::{c_void, CString},
    mem,
};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{GetLastError, BOOL, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{WindowFromDC, HDC},
        System::{
            Console::AllocConsole,
            LibraryLoader::{GetModuleHandleA, GetProcAddress},
            SystemServices::DLL_PROCESS_ATTACH,
        },
        UI::WindowsAndMessaging::{
            CallWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC, GWL_WNDPROC, WA_INACTIVE,
            WHEEL_DELTA, WM_ACTIVATE, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP,
            WM_MOUSEHWHEEL, WM_MOUSEWHEEL, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP,
            WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDBLCLK, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1,
        },
    },
};

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    _module: HINSTANCE,
    call_reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    if call_reason == DLL_PROCESS_ATTACH {
        BOOL::from(main().is_ok())
    } else {
        BOOL::from(true)
    }
}

fn create_debug_console() -> Result<()> {
    if !unsafe { AllocConsole() }.as_bool() {
        return Err(anyhow!(
            "Failed allocating console, GetLastError: {}",
            unsafe { GetLastError() }.0
        ));
    }

    Ok(())
}

fn get_module_library(
    module: &str,
    function: &str,
) -> Result<unsafe extern "system" fn() -> isize> {
    let module_cstring = CString::new(module).expect("module");
    let function_cstring = CString::new(function).expect("function");

    let h_instance = unsafe { GetModuleHandleA(PCSTR(module_cstring.as_ptr() as *mut _)) }?;

    let func = unsafe { GetProcAddress(h_instance, PCSTR(function_cstring.as_ptr() as *mut _)) };

    match func {
        Some(func) => Ok(func),
        None => Err(anyhow!(
            "Failed GetProcAddress, GetLastError: {}",
            unsafe { GetLastError() }.0
        )),
    }
}

static_detour! {
  pub static OpenGl32wglSwapBuffers: unsafe extern "system" fn(HDC) -> ();
}

static mut INIT: bool = false;
static mut IMGUI: Option<Context> = None;
static mut IMGUI_RENDERER: Option<Renderer> = None;
static mut ORIG_HWND: Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT> =
    None;

fn loword(l: u32) -> u16 {
    (l & 0xffff) as u16
}

fn hiword(l: u32) -> u16 {
    ((l >> 16) & 0xffff) as u16
}

fn get_wheel_delta_wparam(wparam: u32) -> u16 {
    hiword(wparam) as u16
}

fn get_xbutton_wparam(wparam: u32) -> u16 {
    hiword(wparam)
}

fn imgui_wnd_proc_impl(
    hwnd: HWND,
    umsg: u32,
    WPARAM(wparam): WPARAM,
    LPARAM(lparam): LPARAM,
) -> LRESULT {
    let mut io = unsafe { IMGUI.as_mut().unwrap() }.io_mut();
    println!("Got msg: {}", umsg);
    match umsg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if wparam < 256 {
                io.keys_down[wparam as usize] = true;
            }
        }
        WM_KEYUP | WM_SYSKEYUP => {
            if wparam < 256 {
                io.keys_down[wparam as usize] = false;
            }
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            io.mouse_down[0] = true;
        }
        WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
            io.mouse_down[1] = true;
        }
        WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
            io.mouse_down[2] = true;
        }
        WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
            let btn = if hiword(wparam as _) == XBUTTON1.0 as u16 {
                3
            } else {
                4
            };
            io.mouse_down[btn] = true;
        }
        WM_LBUTTONUP => {
            io.mouse_down[0] = false;
        }
        WM_RBUTTONUP => {
            io.mouse_down[1] = false;
        }
        WM_MBUTTONUP => {
            io.mouse_down[2] = false;
        }
        WM_XBUTTONUP => {
            let btn = if hiword(wparam as _) == XBUTTON1.0 as u16 {
                3
            } else {
                4
            };
            io.mouse_down[btn] = false;
        }
        WM_MOUSEWHEEL => {
            let wheel_delta_wparam = get_wheel_delta_wparam(wparam as _);
            let wheel_delta = WHEEL_DELTA as f32;
            io.mouse_wheel += (wheel_delta_wparam as i16 as f32) / wheel_delta;
        }
        WM_MOUSEHWHEEL => {
            let wheel_delta_wparam = get_wheel_delta_wparam(wparam as _);
            let wheel_delta = WHEEL_DELTA as f32;
            io.mouse_wheel_h += (wheel_delta_wparam as i16 as f32) / wheel_delta;
        }
        WM_CHAR => io.add_input_character(wparam as u8 as char),
        WM_ACTIVATE => {
            //*imgui_renderer.focus_mut() = loword(wparam as _) != WA_INACTIVE as u16;
            return LRESULT(1);
        }
        _ => {}
    };

    /*let wnd_proc = imgui_renderer.wnd_proc();
    let should_block_messages = imgui_render_loop
        .as_ref()
        .should_block_messages(imgui_renderer.io());
    drop(imgui_renderer);*/

    LRESULT(1)
    //unsafe { CallWindowProcW(ORIG_HWND, hwnd, umsg, WPARAM(wparam), LPARAM(lparam)) }
}

#[allow(non_snake_case)]
fn wndproc_hook(hWnd: HWND, uMsg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
    //println!("Msg is: {}", uMsg);

    imgui_wnd_proc_impl(hWnd, uMsg, wParam, lParam)
    //unsafe { CallWindowProcW(ORIG_HWND, hWnd, uMsg, wParam, lParam) }
}

#[allow(non_snake_case)]
pub fn wglSwapBuffers_detour(dc: HDC) -> () {
    //println!("Called wglSwapBuffers");

    if !unsafe { INIT } {
        let game_window = unsafe { WindowFromDC(dc) };

        unsafe {
            ORIG_HWND = mem::transmute::<
                isize,
                Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>,
            >(SetWindowLongPtrW(
                game_window,
                GWL_WNDPROC,
                wndproc_hook as isize,
            ))
        };

        //hGameWindowProc = (WNDPROC)SetWindowLongPtr(hGameWindow,
        //   GWLP_WNDPROC, (LONG_PTR)windowProc_hook);

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);

        imgui.style_mut().window_title_align = [0.5, 0.5];
        imgui.io_mut().display_size = [1024.0, 1024.0];

        // Init the loader (grabbing the func required)
        gl_loader::init_gl();
        // Create the renderer
        let renderer = imgui_opengl_renderer::Renderer::new(&mut imgui, |s| {
            gl_loader::get_proc_address(s) as _
        });

        unsafe { IMGUI = Some(imgui) };
        unsafe { IMGUI_RENDERER = Some(renderer) };

        unsafe { INIT = true };
    }

    if unsafe { INIT } {
        let imgui = unsafe { &mut IMGUI }.as_mut().unwrap();
        let ui = imgui.frame();
        ui.show_demo_window(&mut true);

        let rendererer = unsafe { &mut IMGUI_RENDERER }.as_mut().unwrap();
        rendererer.render(ui);
    }

    //println!("INIT: {}", unsafe { INIT });

    /*let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);

    let renderer =
        imgui_opengl_renderer::Renderer::new(&mut imgui, |s| gl_get_proc_address(s) as _);

    let mut last_frame = Instant::now();

    loop {
        let now = Instant::now();
        let delta = now - last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        last_frame = now;
        imgui.io_mut().delta_time = delta_s;

        let ui = imgui.frame();
        ui.show_demo_window(&mut true);

        renderer.render(ui);

        ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 60));
    }*/

    unsafe { OpenGl32wglSwapBuffers.call(dc) }
}

pub type FnOpenGl32wglSwapBuffers = unsafe extern "system" fn(HDC) -> ();

fn main() -> Result<()> {
    create_debug_console()?;
    println!("Created debug console");

    let x = get_module_library("opengl32.dll", "wglSwapBuffers")?;
    let y: FnOpenGl32wglSwapBuffers = unsafe { mem::transmute(x) };
    unsafe { OpenGl32wglSwapBuffers.initialize(y, wglSwapBuffers_detour) }?;
    println!("Initialized detour");

    unsafe { OpenGl32wglSwapBuffers.enable() }?;
    println!("Enabled detour");

    Ok(())
}
