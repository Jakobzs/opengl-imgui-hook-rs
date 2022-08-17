extern crate gl;
extern crate imgui;
extern crate imgui_opengl_renderer;
extern crate imgui_sdl2;
extern crate sdl2;

use std::{ffi::CString, ptr, time::Instant};

fn gl_get_proc_address(procname: &str) -> *const () {
    // For reference on what we do here: https://github.com/Rebzzel/kiero/blob/master/kiero.cpp#L519
    println!("Proc address: {}", procname);
    match CString::new(procname) {
        Ok(procname) => unsafe {
            // TODO: Get proc address and retrieve ptr to the function
            // sys::SDL_GL_GetProcAddress(procname.as_ptr() as *const c_char) as *const ()
            ptr::null()
        },
        // string contains a nul byte - it won't match anything.
        Err(_) => ptr::null(),
    }
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    {
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 0);
    }

    let window = video
        .window("rust-imgui-sdl2 demo", 1000, 1000)
        .position_centered()
        .resizable()
        .opengl()
        .allow_highdpi()
        .build()
        .unwrap();

    let _gl_context = window
        .gl_create_context()
        .expect("Couldn't create GL context");
    gl::load_with(|s| video.gl_get_proc_address(s) as _);

    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);

    let mut imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut imgui, &window);

    let renderer =
        imgui_opengl_renderer::Renderer::new(&mut imgui, |s| gl_get_proc_address(s) as _);

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut last_frame = Instant::now();

    'running: loop {
        use sdl2::event::Event;
        use sdl2::keyboard::Keycode;

        for event in event_pump.poll_iter() {
            imgui_sdl2.handle_event(&mut imgui, &event);
            if imgui_sdl2.ignore_event(&event) {
                continue;
            }

            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        imgui_sdl2.prepare_frame(imgui.io_mut(), &window, &event_pump.mouse_state());

        let now = Instant::now();
        let delta = now - last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        last_frame = now;
        imgui.io_mut().delta_time = delta_s;

        let ui = imgui.frame();
        ui.show_demo_window(&mut true);

        unsafe {
            gl::ClearColor(0.2, 0.2, 0.2, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        imgui_sdl2.prepare_render(&ui, &window);
        renderer.render(ui);

        window.gl_swap_window();

        ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 60));
    }
}
