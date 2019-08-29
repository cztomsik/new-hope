#![allow(unused_variables, dead_code)]

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

/// 2D Point, or a vector of movement
#[derive(Clone, Copy)]
struct Pos(Au, Au);

/// Everything what's rendered, is quad-based, it's way easier to imagine then
struct Quad<T>([Vertex<T>; 4]);

/// Vertex including some primitive-specific attributes
struct Vertex<T>(Pos, T);

// for indexed drawing
// raspi can do only 65k vertices in one batch
// could be configurable but it's probably better to play it safe
type VertexIndex = u16;

/// Colors are RGBA, we could save 4x8 bits for each opaque quad but
/// it's probably not worth the additional complexity and we can share
/// buffer for opaque/alpha quads then (faster opacity animation)
#[derive(Clone, Copy)]
struct RGBA(u8, u8, u8, u8);

struct Buffer<T> {
    vbo: VboId,
    data: LeakyVec<T>
}

impl <T> Buffer<T> {
    fn new() -> Self {
        let mut vbo = 0;

        unsafe { gl::GenBuffers(1, &mut vbo) }

        Self {
            vbo,
            data: LeakyVec::new()
        }
    }

    fn remove(&mut self, id: usize) {
        self.data.remove(id);
    }
}

impl <T: Copy> Buffer<Quad<T>> {
    fn add_quad(&mut self, a: Pos, b: Pos, data: T) -> QuadId {
        let quad = Quad([
            Vertex(a, data),
            Vertex(b, data),
            Vertex(Pos(a.0, b.1), data),
            Vertex(Pos(a.1, b.0), data),
        ]);

        self.data.add(quad)
    }

    fn set_quad_bounds(&mut self, id: QuadId, a: Pos, b: Pos) {
        let q = &mut self.data[id];

        q.0[0].0 = a;
        q.0[1].0 = b;
        q.0[2].0 = Pos(a.0, b.1);
        q.0[3].0 = Pos(a.1, b.0);
    }
}

type QuadId = usize;
type BatchId = usize;
type RectId = usize;
type BufferId = usize;
type ImageId = (BatchId, QuadId);
type TextId = (BatchId, BufferId);

struct Batch {
    program: i32,
    uniforms: Vec<Uniform>
}

impl Batch {
    fn new() -> Self {
        Self { program: 0, uniforms: Vec::new() }
    }
}

enum Uniform {
    Float(f32),
    Float2(f32, f32),
    Float3(f32, f32, f32),
}

type VboId = u32;

struct NotSureWhat {
    rect_program: u32,
    image_program: u32,
    text_program: u32,

    rect_buffer: Buffer<Quad<RGBA>>,
    image_buffer: Buffer<Quad<RGBA>>,
    text_buffers: LeakyVec<Buffer<Quad<Pos>>>,

    batches: LeakyVec<Batch>
}

/// stateful, low-level renderer
/// not meant to be used directly
///
/// for each "primitive" we support some of these operations:
/// - add/remove (to render anything)
/// - bounds/position and/or dimension changes (if something is pushing items when expanded, etc.) 
/// - color (hover)
impl NotSureWhat {
    fn new() -> Self {
        unsafe {
            // not used but webgl & opengl core profile require it
            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            check();

            Self {
                rect_program: shader_program(RECT_VS, RECT_FS),
                image_program: shader_program(IMAGE_VS, IMAGE_FS),
                text_program: shader_program(TEXT_VS, TEXT_FS),

                rect_buffer: Buffer::new(),
                image_buffer: Buffer::new(),
                text_buffers: LeakyVec::new(),

                batches: LeakyVec::new()
            }
        }

    }

    fn add_rect(&mut self, a: Pos, b: Pos, color: RGBA) -> RectId {
        self.rect_buffer.add_quad(a, b, color)

        // TODO: add to batch
    }

    fn set_rect_bounds(&mut self, id: RectId, a: Pos, b: Pos) {
        self.rect_buffer.set_quad_bounds(id, a, b);
    }

    fn set_rect_color(&mut self, id: RectId, color: RGBA) {
        // self.rect_buffer[id].
    }

    fn remove_rect(&mut self, id: RectId) {
        self.rect_buffer.remove(id);

        // TODO: remove from indices (from appropriate batch)
    }

    // TODO: image texture
    fn add_image(&mut self, a: Pos, b: Pos) -> ImageId {
        let batch = Batch::new();

        let batch_id = self.batches.add(batch);
        // TODO
        let quad_id = 0;

        (batch_id, quad_id)
    }

    fn set_image_pos(&mut self, id: ImageId, pos: Pos) {
        self.batches[id.0].uniforms[0] = Uniform::Float2(pos.0, pos.1);
    }

    fn remove_image(&mut self, id: ImageId) {
        let (batch_id, quad_id) = id;

        self.batches.remove(batch_id);
        self.image_buffer.remove(quad_id);

        // TODO: batch order
    }

    // TODO: glyphs: &[GlyphType]
    fn add_text(&mut self, pos: Pos, glyphs: usize, color: RGBA) -> TextId {
        let buffer = Buffer::new();

        let buffer_id = self.text_buffers.add(buffer);

        let batch = Batch::new();

        /*
        for _ in 0..text.len() {
            buf.push(...)
            x += 10
        }
        */

        let batch_id = self.batches.add(batch);

        // TODO: batch order

        (batch_id, buffer_id)
    }

    fn set_text_pos(&mut self, id: TextId, pos: Pos) {
        self.batches[id.0].uniforms[0] = Uniform::Float2(pos.0, pos.1);
    }

    fn set_text_color(&mut self, id: TextId, color: RGBA) {
        self.batches[id.0].uniforms[1] = Uniform::Float3(color.0 as f32, color.1 as f32, color.2 as f32);
    }

    fn remove_text(&mut self, id: TextId) {
        let (batch_id, buffer_id) = id;

        self.batches.remove(batch_id);
        self.text_buffers.remove(buffer_id);

        // TODO: create & delete index buffer (that can be part of batch, but sometimes indices can lead to the same buffer

        // TODO: batch order
    }

    // TODO: skip up-to-date buffers
    unsafe fn upload_buffers(&self) {
      let some_size = mem::size_of::<Vertex<RGBA>>();

      for b in [&self.rect_buffer].iter() {
          gl::BindBuffer(gl::ARRAY_BUFFER, b.vbo);
          gl::BufferData(
            gl::ARRAY_BUFFER,
            (4 * b.data.data.len() * some_size) as isize,
            mem::transmute(&b.data.data[0]),
            gl::STATIC_DRAW
          );
          check();
      }
    }

    fn render(&mut self) {
        unsafe {
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            self.upload_buffers();

            gl::BindBuffer(gl::ARRAY_BUFFER, self.rect_buffer.vbo);
            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(
                0,
                2,
                gl::FLOAT,
                gl::FALSE,
                (mem::size_of::<Vertex<RGBA>>()) as GLint,
                0 as *const GLvoid,
            );

            // TODO: batches
            gl::UseProgram(self.rect_program);

            //println!("{} {}", self.rect_buffer.data.data.len(), mem::size_of::<Vertex<RGBA>>());

            // TODO: indexed drawing
            gl::DrawArrays(gl::TRIANGLES, 0, 3 * self.rect_buffer.data.data.len() as i32);

            check()
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

    let mut renderer = NotSureWhat::new();

    // demo
    let rect = renderer.add_rect(Pos(0., 0., ), Pos(0.5, 0.5), RGBA(0, 0, 0, 255));

    let mut time = Instant::now();
    let mut frames: u128 = 0;
    let mut n: f32 = 0.;

    loop {
        for e in event_pump.poll_iter() {
            match e {
                sdl2::event::Event::Quit { .. } => panic!("TODO: quit"),
                _ => {}
            }
        }

        n += 0.01;
        renderer.set_rect_bounds(rect, Pos(0., n.sin()), Pos(0.5, 0.5));

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

const RECT_VS: &str = r#"
  #version 100

  attribute vec2 a_pos;

  void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
  }
"#;

const RECT_FS: &str = r#"
  #version 100

  precision mediump float;

  void main() {
    gl_FragColor = vec4(0., 0., 0., 1.);
  }
"#;

const IMAGE_VS: &str = RECT_VS;
const IMAGE_FS: &str = RECT_FS;
const TEXT_VS: &str = RECT_VS;
const TEXT_FS: &str = RECT_FS;

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

// some store with stable ids
// TODO: freelist or something (now it just leaks memory)
struct LeakyVec<T> {
    data: Vec<T>
}

impl <T> LeakyVec<T> {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn add(&mut self, item: T) -> usize {
        let id = self.data.len();

        self.data.push(item);

        id
    }

    fn remove(&mut self, id: usize) {}
}

impl <T> std::ops::Index<usize> for LeakyVec<T> {
    type Output = T;

    fn index(&self, key: usize) -> &T {
        &self.data[key]
    }
}

impl <T> std::ops::IndexMut<usize> for LeakyVec<T> {
    fn index_mut(&mut self, key: usize) -> &mut T {
        &mut self.data[key]
    }
}
