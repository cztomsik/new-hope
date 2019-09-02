use std::time::Instant;
use sdl2;
use sdl2::video::GLProfile;

use new_hope::*;

struct Demo {
    time: f32,
    renderer: NotSureWhat,

    managed: (RectId, RectId)
}

impl Demo {
    fn new() -> Self {
        let mut renderer = NotSureWhat::new();

        // demo
        let rect1 = renderer.create_rect(Pos(0., 0.), Pos(1., 1.), RGBA(0, 0, 255, 255));
        let rect2 = renderer.create_rect(Pos(-1., -1.), Pos(0., 0.), RGBA(255, 0, 0, 255));
        let rect3 = renderer.create_rect(Pos(-0.5, -0.5), Pos(0.5, 0.5), RGBA(0, 0, 0, 64));
        let text = renderer.create_text(Pos(0., 0.), 10, RGBA(0, 0, 0, 120));

        renderer.set_display_list(&[
            DisplayItem::Rect(rect1),
            DisplayItem::Rect(rect2),
            DisplayItem::Rect(rect3),
            DisplayItem::Text(text),
        ]);

        Self {
            time: 0.,
            renderer,

            managed: (rect1, rect2)
        }
    }

    fn tick(&mut self, delta: f32) {
        self.time += delta;

        self.renderer.set_rect_bounds(self.managed.0, Pos(0., self.time.sin()), Pos(0.5, 0.5));
        self.renderer.set_rect_bounds(self.managed.1, Pos(self.time.sin(), 0.), Pos(self.time.cos(), 0.5));
    }

    fn render(&mut self) {
        self.renderer.render();
    }
}

fn main() {
    let sdl = sdl2::init().expect("init SDL2");
    let video = sdl.video().expect("init video");
    let mut event_pump = sdl.event_pump().expect("init event pump");

    let window = video
        .window("Test", WIDTH, 900)
        .opengl()
        .resizable()
        .build()
        .expect("init window");

    if !EMSCRIPTEN {
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(GLProfile::Core);        
    }

    let gl_context = window.gl_create_context().expect("create context");
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    window.gl_make_current(&gl_context).expect("set context");

    // raspi never waits for vsync but turning it off throws
    let _ = video.gl_set_swap_interval(sdl2::video::SwapInterval::Immediate);

    let mut time = Instant::now();
    let mut frames: u128 = 0;

    let mut demo = Demo::new();

    loop {
        for e in event_pump.poll_iter() {
            match e {
                sdl2::event::Event::Quit { .. } => panic!("TODO: quit"),
                _ => {}
            }
        }

        demo.tick(0.002);
        demo.render();
        window.gl_swap_window();

        let elapsed = time.elapsed().as_nanos() as f32 / 1_000_000_000 as f32;

        if elapsed > 5. {
            // BTW: make sure to hide terminal & other windows, sometimes it can do wonders with FPS
            println!("avg FPS {}", frames as f32 / elapsed);
            frames = 0;
            time = Instant::now();
        }

        frames += 1;

        // sleep a bit
        // ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 100));
    }
}

const WIDTH: u32 = 1200;

#[cfg(target_os = "emscripten")]
static EMSCRIPTEN: bool = true;
#[cfg(not(target_os = "emscripten"))]
static EMSCRIPTEN: bool = false;
