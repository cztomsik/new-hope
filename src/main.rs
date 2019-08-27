use std::ffi::CString;
use std::time::Instant;

use std::mem;
use std::ptr;
use std::str;

use gl::types::*;
use sdl2;
use sdl2::video::GLProfile;

/// Application unit (or something similar, unit of measure)
/// TODO(later): Integer type could save some CPU & memory
type Au = f32;

/// Everything is quad-based, because it's way easier to imagine then
struct Quad<T>([Vertex<T>; 4]);

/// Vertex including some primitive-specific attributes
struct Vertex<T>(Au, Au, T);

// for indexed drawing
// raspi can do only 65k vertices in one batch
// could be configurable but it's probably better to play it safe
//type VertexIndex = u16;

/// Colors are RGBA, we could save 4x8 bits for each opaque quad but
/// it's probably not worth the additional complexity and we can share
/// buffer for opaque/alpha quads then (faster opacity animation)
#[derive(Clone, Copy)]
struct RGBA(u8, u8, u8, u8);

struct Buffer<T> {
    vbo: VboId,
    data: Vec<T>
}

impl <T> Buffer<T> {
    fn new() -> Self {
        let mut vbo = 0;

        unsafe { gl::GenBuffers(1, &mut vbo) }

        Self {
            vbo,
            data: Vec::new()
        }
    }
}

impl <T: Copy> Buffer<Quad<T>> {
    // TODO: a, b points
    fn add_quad(&mut self, (x, y, w, h): (Au, Au, Au, Au), data: T) {
        let quad = Quad([
            Vertex(x, y, data),
            Vertex(x + w, y, data),
            Vertex(x + w, y + h, data),
            Vertex(x, y + h, data)
        ]);

        self.data.push(quad);
    }
}

type VboId = u32;

struct NotSureWhat {
    some_buf: Buffer<Quad<RGBA>>
}

impl NotSureWhat {
    fn new() -> Self {
        let mut res = Self {
            some_buf: Buffer::new()
        };

        res.some_buf.add_quad((0., 0., 0.5, 0.5), RGBA(0, 0, 0, 255));

        res
    }

    // TODO: skip up-to-date buffers
    unsafe fn upload_buffers(&self) {
      let some_size = mem::size_of::<Vertex<RGBA>>();

      for b in [&self.some_buf].iter() {
          gl::BindBuffer(gl::ARRAY_BUFFER, b.vbo);
          gl::BufferData(
            gl::ARRAY_BUFFER,
            (4 * b.data.len() * some_size) as isize,
            mem::transmute(&b.data[0]),
            gl::STATIC_DRAW
          );
          check();
      }
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

    let mut renderer = GlRenderer::new();

    let mut time = Instant::now();
    let mut frames: u128 = 0;

    loop {
        for e in event_pump.poll_iter() {
            match e {
                sdl2::event::Event::Quit { .. } => panic!("TODO: quit"),
                _ => {}
            }
        }

        renderer.render();
        window.gl_swap_window();

        let elapsed = time.elapsed().as_nanos() as f32 / 1_000_000_000 as f32;

        if elapsed > 1. {
            // BTW: make sure to hide terminal & other windows, sometimes it can do wonders with FPS
            println!("avg FPS {}", frames as f32 / elapsed);
            frames = 0;
            time = Instant::now();
        }

        frames += 1;

        // limit to 100 FPS
        //::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 100));
    }
}

const WIDTH: u32 = 1200;

struct GlRenderer {
    foo: NotSureWhat
}

impl GlRenderer {
    fn new() -> Self {
        init();
        let foo = NotSureWhat::new();

        Self { foo }
    }

    fn render(&mut self) {
        unsafe {
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            self.foo.upload_buffers();

            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(
                0,
                2,
                gl::FLOAT,
                gl::FALSE,
                (mem::size_of::<Vertex<RGBA>>()) as GLint,
                0 as *const GLvoid,
            );

            println!("{}", mem::size_of::<Vertex<RGBA>>());
            gl::DrawArrays(gl::TRIANGLES, 0, 3 * self.foo.some_buf.data.len() as i32);

            check()
        }
    }
}

const VERTEX_SHADER_SOURCE: &str = r#"
  #version 100

  attribute vec2 a_pos;

  void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
  }
"#;

const FRAGMENT_SHADER_SOURCE: &str = r#"
  #version 100

  precision mediump float;

  void main() {
    gl_FragColor = vec4(0., 0., 0., 1.);
  }
"#;

fn init() {
    unsafe {
        // not used but webgl & opengl core profile requires it
        let mut vao = 0;
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);

        let shader_program = shader_program(VERTEX_SHADER_SOURCE, FRAGMENT_SHADER_SOURCE);
        gl::UseProgram(shader_program);

        check();
    }
}

unsafe fn check() {
    let err = gl::GetError();
    if err != gl::NO_ERROR {
        panic!("gl err {}", err);
    }
}

// from gleam
fn get_shader_info_log(shader: GLuint) -> String {
    let mut max_len = [0];
    unsafe {
        get_shader_iv(shader, gl::INFO_LOG_LENGTH, &mut max_len);
    }
    if max_len[0] == 0 {
        return String::new();
    }
    let mut result = vec![0u8; max_len[0] as usize];
    let mut result_len = 0 as GLsizei;
    unsafe {
        gl::GetShaderInfoLog(
            shader,
            max_len[0] as GLsizei,
            &mut result_len,
            result.as_mut_ptr() as *mut GLchar,
        );
    }
    result.truncate(if result_len > 0 {
        result_len as usize
    } else {
        0
    });
    String::from_utf8(result).unwrap()
}
unsafe fn get_shader_iv(shader: GLuint, pname: GLenum, result: &mut [GLint]) {
    assert!(!result.is_empty());
    gl::GetShaderiv(shader, pname, result.as_mut_ptr());
}
fn get_program_info_log(program: GLuint) -> String {
    let mut max_len = [0];
    unsafe {
        get_program_iv(program, gl::INFO_LOG_LENGTH, &mut max_len);
    }
    if max_len[0] == 0 {
        return String::new();
    }
    let mut result = vec![0u8; max_len[0] as usize];
    let mut result_len = 0 as GLsizei;
    unsafe {
        gl::GetProgramInfoLog(
            program,
            max_len[0] as GLsizei,
            &mut result_len,
            result.as_mut_ptr() as *mut GLchar,
        );
    }
    result.truncate(if result_len > 0 {
        result_len as usize
    } else {
        0
    });
    String::from_utf8(result).unwrap()
}
unsafe fn get_program_iv(program: GLuint, pname: GLenum, result: &mut [GLint]) {
    assert!(!result.is_empty());
    gl::GetProgramiv(program, pname, result.as_mut_ptr());
}

unsafe fn shader_program(vertex_shader_source: &str, fragment_shader_source: &str) -> u32 {
    let vertex_shader = shader(gl::VERTEX_SHADER, vertex_shader_source);
    let fragment_shader = shader(gl::FRAGMENT_SHADER, fragment_shader_source);

    let program = gl::CreateProgram();
    gl::AttachShader(program, vertex_shader);
    gl::AttachShader(program, fragment_shader);
    gl::LinkProgram(program);

    let mut success = gl::FALSE as GLint;

    gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);

    if success != gl::TRUE as GLint {
        panic!(get_program_info_log(program));
    }

    gl::DeleteShader(vertex_shader);
    gl::DeleteShader(fragment_shader);

    program
}

unsafe fn shader(shader_type: u32, source: &str) -> u32 {
    let shader = gl::CreateShader(shader_type);

    gl::ShaderSource(
        shader,
        1,
        &(CString::new(source.as_bytes()).expect("get CString")).as_ptr(),
        ptr::null(),
    );
    gl::CompileShader(shader);

    let mut success = gl::FALSE as GLint;

    gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);

    if success != gl::TRUE as GLint {
        panic!(get_shader_info_log(shader));
    }

    shader
}

#[cfg(target_os = "emscripten")]
static EMSCRIPTEN: bool = true;
#[cfg(not(target_os = "emscripten"))]
static EMSCRIPTEN: bool = false;
