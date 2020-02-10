#[cfg(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ),
    target_arch = "x86_64"
))]
#[macro_use]
extern crate log;

#[cfg(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ),
    target_arch = "x86_64"
))]
pub mod support;

#[cfg(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ),
    target_arch = "x86_64"
))]
mod implementation {
    use super::support;

    use drm::control::connector::Info as ConnectorInfo;
    use drm::control::{crtc, framebuffer, Device as ControlDevice, Mode, ResourceInfo};
    use drm::Device as BasicDevice;
    use gbm::{BufferObjectFlags, Device, DeviceGlutinWrapper, Format};
    use glutin::config::{Config, ConfigsFinder};
    use glutin::context::ContextBuilder;
    use glutin::platform::unix::{ConfigExt, RawConfig, RawDisplay};
    use glutin::surface::Surface;
    use libloading;
    use winit_types::dpi::PhysicalSize;

    use std::fs::{File, OpenOptions};
    use std::os::unix::io::{AsRawFd, RawFd};

    struct Card(File);

    impl AsRawFd for Card {
        fn as_raw_fd(&self) -> RawFd {
            self.0.as_raw_fd()
        }
    }

    impl BasicDevice for Card {}
    impl ControlDevice for Card {}

    fn init_drm_device() -> Card {
        let mut options = OpenOptions::new();
        options.read(true);
        options.write(true);
        let file = options.open("/dev/dri/card0").unwrap();
        Card(file)
    }

    unsafe fn choose_conf<'a, T: AsRawFd>(
        gbm: &Device<T>,
        confs: &'a [Config],
        flags: BufferObjectFlags,
    ) -> &'a Config {
        let lib = libloading::Library::new("libEGL.so.1")
            .unwrap_or_else(|_| libloading::Library::new("libEGL.so").unwrap());
        let egl = glutin_egl_sys::egl::Egl::load_with(|f| support::load_egl_sym(&lib, f));

        for conf in confs {
            let raw_conf = match conf.raw_config() {
                RawConfig::Egl(conf) => conf,
                _ => panic!(),
            };

            let raw_disp = match conf.raw_display() {
                RawDisplay::Egl(disp) => disp,
                _ => panic!(),
            };

            let mut format = 0;
            const NATIVE_VISUAL_ID: i32 = 0x302E;
            if egl.GetConfigAttrib(raw_disp, raw_conf, NATIVE_VISUAL_ID, &mut format) == 0 {
                warn!(
                    "Failed to get NATIVE_VISUAL_ID for disp {:?} w/ conf {:?}, err {:?}",
                    raw_disp,
                    raw_conf,
                    egl.GetError()
                );
            } else {
                match Format::from_ffi(format as _) {
                    Some(format) if gbm.is_format_supported(format, flags) => return conf,
                    Some(format) => warn!(
                        "{:?}'s format {:?} incompatible with flags",
                        raw_conf, format
                    ),
                    None => warn!(
                        "Skipped over {:?} as format {:?} unkown to gbm-rs",
                        raw_conf, format
                    ),
                }
            }
        }

        panic!()
    }

    pub fn main() {
        simple_logger::init().unwrap();
        let drm = init_drm_device();
        let gbm = Device::new(drm).unwrap();

        let res_handles = gbm.resource_handles().unwrap();
        let con = *res_handles.connectors().iter().next().unwrap();
        let crtc_handle = *res_handles.crtcs().iter().next().unwrap();
        let connector_info: ConnectorInfo = gbm.resource_info(con).unwrap();
        let mode: Mode = connector_info.modes()[0];
        let dims = mode.size();
        let dims = PhysicalSize::new(dims.0 as u32, dims.1 as u32);
        let flags = BufferObjectFlags::SCANOUT | BufferObjectFlags::RENDERING;

        let confs = unsafe { ConfigsFinder::new().find(&gbm).unwrap() };
        let conf = unsafe { choose_conf(&gbm, &confs, flags) };
        println!("Configeration chosen: {:?}", conf);

        let ctx = unsafe { ContextBuilder::new().build(conf).unwrap() };
        let (gbmsurf, surf) = unsafe {
            let gbm: DeviceGlutinWrapper<_, ()> = (&gbm).into();
            Surface::new_window(conf, &gbm, (dims, flags)).unwrap()
        };
        unsafe { ctx.make_current(&surf).unwrap() }
        let gl = support::Gl::load(|s| ctx.get_proc_address(s).unwrap());

        let mut has_modsetted = false;
        loop {
            gl.draw_frame([1.0, 0.5, 0.7, 1.0]);
            surf.swap_buffers().unwrap();
            let bo = unsafe { gbmsurf.lock_front_buffer().unwrap() };
            let fb_info = framebuffer::create(&gbm, &*bo).unwrap();
            if !has_modsetted {
                crtc::set(
                    &gbm,
                    crtc_handle,
                    fb_info.handle(),
                    &[con],
                    (0, 0),
                    Some(mode),
                )
                .unwrap();
                has_modsetted = true;
            }
            crtc::page_flip(
                &gbm,
                crtc_handle,
                fb_info.handle(),
                &[crtc::PageFlipFlags::PageFlipEvent],
            )
            .unwrap();
            for e in crtc::receive_events(&gbm).unwrap() {
                match e {
                    crtc::Event::PageFlip(_) => break,
                    _ => (),
                }
            }
        }
    }
}

#[cfg(not(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ),
    target_arch = "x86_64"
)))]
mod implementation {
    pub fn main() {
        panic!("This example is for linux only.")
    }
}

fn main() {
    implementation::main();
}