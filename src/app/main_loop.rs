use crate::app::App;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::timer::{delay, ticks};
use std::sync::mpsc;

#[cfg(target_os = "macos")]
use crate::app::menu_bar::{MenuBar, MenuCommand};

#[cfg(feature = "dummy_audio")]
use crate::dummy_audio;

/// Commands sent from the main (event) thread to the render thread.
enum RenderCmd {
    SwitchAudioDevice(sdl3::audio::AudioDeviceID),
    SwitchToNextDevice,
    NextPreset,
    PrevPreset,
    RandomPreset,
    /// Re-query the window pixel size and update projectM. Sent after any
    /// fullscreen or resize change detected on the main thread.
    SyncWindowSize,
    Quit,
}

impl App {
    pub fn main_loop(&mut self) {
        let frame_rate = self.config.frame_rate.unwrap();

        // Menu bar lives on the main thread (AppKit requirement on macOS).
        #[cfg(target_os = "macos")]
        let mut menu_bar = {
            let devices = self.audio.get_recording_devices();
            let current = self.audio.recording_device_name();
            MenuBar::new(frame_rate, &devices, current.as_deref())
        };

        let (cmd_tx, cmd_rx) = mpsc::channel::<RenderCmd>();
        // Render thread sends back device names so the menu bar can reflect changes.
        let (name_tx, name_rx) = mpsc::sync_channel::<String>(4);

        // -----------------------------------------------------------------------
        // Hand the render-side state to a background thread via raw pointers.
        //
        // SAFETY invariants that make this sound:
        //   • `self` (and therefore every field) lives until `main_loop` returns.
        //   • `main_loop` only returns after `render_handle.join()`, so all raw
        //     pointers remain valid for the render thread's entire lifetime.
        //   • After spawning, the main thread touches only `sdl_context` and
        //     `menu_bar`; the render thread exclusively owns pm / playlist /
        //     audio / window / gl_context.
        // -----------------------------------------------------------------------
        let pm_ptr = std::rc::Rc::as_ptr(&self.pm) as usize;
        let playlist_ptr = &mut self.playlist as *mut _ as usize;
        let audio_ptr = &mut self.audio as *mut _ as usize;
        let window_ptr = self.window.raw() as usize;
        let gl_ctx_ptr = unsafe { self._gl_context.raw() } as usize;

        // Detach the GL context from the main thread before the render thread
        // claims it.
        unsafe {
            sdl3::sys::video::SDL_GL_MakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
        }

        let render_handle = std::thread::spawn(move || {
            use crate::app::audio::Audio;
            use projectm::core::ProjectM;
            use projectm::playlist::Playlist;
            use sdl3::sys::video as sdlv;

            // Reconstruct typed references from the raw pointers.
            // SAFETY: see invariants above.
            let window_raw = window_ptr as *mut sdlv::SDL_Window;
            let gl_ctx_raw = gl_ctx_ptr as sdlv::SDL_GLContext;
            let pm = unsafe { &*(pm_ptr as *const ProjectM) };
            let playlist = unsafe { &mut *(playlist_ptr as *mut Playlist) };
            let audio = unsafe { &mut *(audio_ptr as *mut Audio) };

            // Make the GL context current on this thread.
            unsafe {
                sdlv::SDL_GL_MakeCurrent(window_raw, gl_ctx_raw);
            }

            let mut last_pixel_size = (0usize, 0usize);

            'render: loop {
                let start = ticks();

                // Sync projectM with the current window pixel size every frame.
                // Using change detection so set_window_size is only called when
                // the size actually changes. This catches SDL fullscreen (F key),
                // native macOS fullscreen (⌘F / green button), and window resizes.
                {
                    let (mut w, mut h) = (0i32, 0i32);
                    unsafe {
                        sdlv::SDL_GetWindowSizeInPixels(window_raw, &mut w, &mut h);
                    }
                    let size = (w as usize, h as usize);
                    if size.0 > 0 && size.1 > 0 && size != last_pixel_size {
                        pm.set_window_size(size.0, size.1);
                        last_pixel_size = size;
                    }
                }

                // Drain all pending commands before rendering.
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        RenderCmd::Quit => break 'render,
                        RenderCmd::NextPreset => {
                            playlist.play_next();
                        }
                        RenderCmd::PrevPreset => {
                            playlist.play_prev();
                        }
                        RenderCmd::RandomPreset => {
                            playlist.play_random();
                        }
                        RenderCmd::SwitchAudioDevice(id) => {
                            audio.open_device_by_id(id);
                            if let Some(name) = audio.recording_device_name() {
                                name_tx.try_send(name).ok();
                            }
                        }
                        RenderCmd::SwitchToNextDevice => {
                            audio.open_next_device();
                            if let Some(name) = audio.recording_device_name() {
                                name_tx.try_send(name).ok();
                            }
                        }
                        RenderCmd::SyncWindowSize => {} // handled by per-frame check above
                    }
                }

                #[cfg(feature = "dummy_audio")]
                dummy_audio::generate_random_audio_data(pm);

                #[cfg(not(feature = "dummy_audio"))]
                audio.process_frame_samples();

                pm.render_frame();

                unsafe {
                    sdlv::SDL_GL_SwapWindow(window_raw);
                }

                if frame_rate > 0 {
                    let frame_time: i32 = (ticks() - start).try_into().unwrap();
                    let delay_needed: i32 = 1000 / frame_rate as i32 - frame_time;
                    if delay_needed > 0 {
                        delay(delay_needed as u32);
                    }
                }
            }
        });

        // -----------------------------------------------------------------------
        // Main thread: SDL event pump + menu bar.
        // Rendering is now on the background thread, so menus opening no longer
        // stalls the visuals.
        // -----------------------------------------------------------------------
        let mut event_pump = self.sdl_context.event_pump().unwrap();
        let mut fps_tick = ticks();

        'running: loop {
            // Relay device-name updates from render thread to menu bar.
            if let Ok(name) = name_rx.try_recv() {
                #[cfg(target_os = "macos")]
                menu_bar.update_device(&name);
            }

            // Update FPS label once per second.
            let now = ticks();
            if now.wrapping_sub(fps_tick) >= 1000 {
                fps_tick = now;
                #[cfg(target_os = "macos")]
                menu_bar.update_fps(frame_rate);
            }

            // Poll menu-bar commands and forward to render thread.
            #[cfg(target_os = "macos")]
            match menu_bar.poll_command() {
                Some(MenuCommand::SwitchAudioDevice(id)) => {
                    cmd_tx.send(RenderCmd::SwitchAudioDevice(id)).ok();
                }
                Some(MenuCommand::NextInputDevice) => {
                    cmd_tx.send(RenderCmd::SwitchToNextDevice).ok();
                }
                Some(MenuCommand::NextPreset) => {
                    cmd_tx.send(RenderCmd::NextPreset).ok();
                }
                Some(MenuCommand::PrevPreset) => {
                    cmd_tx.send(RenderCmd::PrevPreset).ok();
                }
                Some(MenuCommand::RandomPreset) => {
                    cmd_tx.send(RenderCmd::RandomPreset).ok();
                }
                None => {}
            }

            // SDL events.
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => {
                        cmd_tx.send(RenderCmd::Quit).ok();
                        break 'running;
                    }
                    Event::KeyUp {
                        keycode: Some(Keycode::N | Keycode::Right),
                        ..
                    } => {
                        cmd_tx.send(RenderCmd::NextPreset).ok();
                    }
                    Event::KeyUp {
                        keycode: Some(Keycode::P | Keycode::Left),
                        ..
                    } => {
                        cmd_tx.send(RenderCmd::PrevPreset).ok();
                    }
                    Event::KeyUp {
                        keycode: Some(Keycode::R),
                        ..
                    } => {
                        cmd_tx.send(RenderCmd::RandomPreset).ok();
                    }
                    // Fullscreen is toggled on the main thread (no GL context held here).
                    // The render thread syncs projectM's window size on the resize event.
                    Event::KeyUp {
                        keycode: Some(Keycode::F),
                        ..
                    } => {
                        let flags =
                            unsafe { sdl3::sys::video::SDL_GetWindowFlags(window_ptr as *mut _) };
                        let is_fs = flags & sdl3::sys::video::SDL_WINDOW_FULLSCREEN != 0;
                        unsafe {
                            sdl3::sys::video::SDL_SetWindowFullscreen(window_ptr as *mut _, !is_fs);
                        }
                    }
                    Event::KeyUp {
                        keycode: Some(Keycode::Q),
                        ..
                    } => {
                        cmd_tx.send(RenderCmd::Quit).ok();
                        break 'running;
                    }
                    // Any pixel-size change (SDL fullscreen, native macOS fullscreen,
                    // or window resize) — tell the render thread to resync projectM.
                    Event::Window {
                        win_event: sdl3::event::WindowEvent::PixelSizeChanged(..),
                        ..
                    } => {
                        cmd_tx.send(RenderCmd::SyncWindowSize).ok();
                    }
                    _ => {}
                }
            }

            // Brief sleep so the main thread doesn't spin at 100 % CPU.
            // The NSMenu modal loop pre-empts this during menu interaction,
            // while the render thread continues unaffected on its own thread.
            delay(1);
        }

        render_handle.join().expect("render thread panicked");
    }
}
