#![allow(unused_variables, dead_code)]

use std::ffi::CString;

use std::mem;
use std::ptr;
use std::str;

use gl::types::*;

/// Application unit (or something similar, unit of measure)
/// TODO(later): Integer type could save some CPU & memory
type Au = f32;

/// 2D Point
#[derive(Clone, Copy, Debug)]
pub struct Pos(pub Au, pub Au);

/// Colors are RGBA, we could save 4x8 bits for each opaque quad but
/// it's probably not worth the additional complexity
#[derive(Clone, Copy, Debug)]
pub struct RGBA(pub u8, pub u8, pub u8, pub u8);

pub struct NotSureWhat {
    rect_program: u32,
    image_program: u32,
    text_program: u32,

    rect_buffer: Buffer<Quad<RGBA>>,
    image_buffer: Buffer<Quad<Pos>>,
    texts: LeakyVec<Text>,

    batches: Vec<Batch>,
    // shared for all batches to save bandwidth
    index_buffer: Buffer<VertexIndex>,
}

/// stateful, low-level renderer
/// not meant to be used directly
///
/// primitives are kept around like in retained mode but the actual
/// rendering order is separated & typically generated in the immediate mode fashion
///
/// the general idea is that small changes (change of color) should be cheap
///
/// for each "primitive" we support some of these operations:
/// - add/remove (to render anything)
/// - bounds/position and/or dimension changes (if something is pushing items when expanded, etc.) 
/// - change color (hover)
impl NotSureWhat {
    pub fn new() -> Self {
        unsafe {
            // not used but webgl & opengl core profile require it
            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            check();

            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::BlendEquation(gl::FUNC_ADD);

            check();

            Self {
                rect_program: shader_program(RECT_VS, RECT_FS),
                image_program: shader_program(IMAGE_VS, IMAGE_FS),
                text_program: shader_program(TEXT_VS, TEXT_FS),

                rect_buffer: Buffer::new(),
                image_buffer: Buffer::new(),
                texts: LeakyVec::new(),

                batches: Vec::new(),
                index_buffer: Buffer::new(),
            }
        }

    }

    pub fn create_rect(&mut self, a: Pos, b: Pos, color: RGBA) -> RectId {
        self.rect_buffer.add_quad(a, b, color)
    }

    pub fn set_rect_bounds(&mut self, id: RectId, a: Pos, b: Pos) {
        self.rect_buffer.set_quad_bounds(id, a, b);
    }

    pub fn set_rect_color(&mut self, id: RectId, color: RGBA) {
        // self.rect_buffer[id].
    }

    pub fn remove_rect(&mut self, id: RectId) {
        self.rect_buffer.remove(id);
    }

    // TODO: image texture
    pub fn create_image(&mut self, a: Pos, b: Pos) -> ImageId {
        // TODO
        0
    }

    pub fn set_image_pos(&mut self, id: ImageId, pos: Pos) {
        //self.batches[id.0].uniforms[0] = Uniform::Float2(pos.0, pos.1);
    }

    pub fn remove_image(&mut self, id: ImageId) {
        // TODO
    }

    // TODO: glyphs: &[GlyphType]
    pub fn create_text(&mut self, pos: Pos, glyphs: usize, color: RGBA) -> TextId {
        let mut buffer = Buffer::new();
        let mut x = 0.;

        let glyph_width = 0.05;
        let glyph_height = 0.1;
        let advance = 0.01;

        // TODO & should be somewhere else
        for _ in 0..glyphs {
            // for now we are rendering just colored quads
            buffer.add_quad(Pos(x, 0.), Pos(x + glyph_width, glyph_height), color);

            //let glyph_uv = Pos(0., 0.);
            //buffer.add_quad(Pos(x, 0.), Pos(x + 9., 16.), glyph_uv);

            x += glyph_width + advance;
        }

        self.texts.add(Text {
            pos, color, buffer
        })
    }

    pub fn set_text_pos(&mut self, id: TextId, pos: Pos) {
        //self.batches[id.0].uniforms[0] = Uniform::Float2(pos.0, pos.1);
    }

    pub fn set_text_color(&mut self, id: TextId, color: RGBA) {
        //self.batches[id.0].uniforms[1] = Uniform::Float3(color.0 as f32, color.1 as f32, color.2 as f32);
    }

    pub fn remove_text(&mut self, id: TextId) {
        self.texts.remove(id);
    }

    // TODO: skip up-to-date buffers
    unsafe fn upload_buffers(&self) {
        self.rect_buffer.upload();

        for t in &self.texts.data {
            t.buffer.upload();
        }
    }

    // if there were changes in the rendering order
    pub fn set_display_list(&mut self, items: &[DisplayItem]) {
        println!("list {:?}", &items);

        let mut batches = Vec::new();
        let mut indices = Vec::new();

        // TODO: fusion
        for it in items {
            match it {
                DisplayItem::Rect(rect_id) => {
                    let base = 4 * (*rect_id as VertexIndex);

                    indices.push(base + 1);
                    indices.push(base);
                    indices.push(base + 3);

                    indices.push(base);
                    indices.push(base + 2);
                    indices.push(base + 3);

                    batches.push(Batch::Rects(1));
                }
                DisplayItem::Text(text_id) => {
                    let text = &self.texts[*text_id];

                    // TODO: this is static and should be generated with glyphs
                    for n in 0..text.buffer.data.data.len() {
                        let base = 4 * (n as VertexIndex);

                        indices.push(base + 1);
                        indices.push(base);
                        indices.push(base + 3);

                        indices.push(base);
                        indices.push(base + 2);
                        indices.push(base + 3);
                    }

                    batches.push(Batch::Text(*text_id));
                }
                _ => unimplemented!()
            }
        }

        unsafe {
            if !indices.is_empty() {
                gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.index_buffer.vbo);
                gl::BufferData(gl::ELEMENT_ARRAY_BUFFER, (indices.len() * mem::size_of::<VertexIndex>()) as GLsizeiptr, mem::transmute(&indices[0]), gl::STATIC_DRAW);
            }
        }

        self.batches = batches;

        // so that memory is freed one day
        self.index_buffer.data.data = indices;
    }

    // most of the work has already been done
    // we just need to go through batches, setup pipeline & do indexed draw
    pub fn render(&mut self) {
        unsafe {
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            self.upload_buffers();

            // maybe in future something more advanced might happen
            // but for now it is hard-coded here

            let mut offset: usize = 0;

            for b in &self.batches {
                let quads_count;

                // println!("batch {:?}", &b);

                match b {
                    Batch::Rects(num_quads) => {
                        gl::UseProgram(self.rect_program);
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
                        gl::EnableVertexAttribArray(1);
                        gl::VertexAttribPointer(
                            1,
                            4,
                            gl::UNSIGNED_BYTE,
                            gl::FALSE,
                            (mem::size_of::<Vertex<RGBA>>()) as GLint,
                            (mem::size_of::<Pos>()) as *const std::ffi::c_void,
                        );

                        quads_count = *num_quads;
                    },
                    Batch::Image => {
                        gl::UseProgram(self.image_program);
                        quads_count = 1;
                    },
                    Batch::Text(text_id) => {
                        gl::UseProgram(self.text_program);
                        // TODO: glyph coords/glyph_index
                        // TODO: uniforms

                        let text = &self.texts[*text_id];

                        gl::BindBuffer(gl::ARRAY_BUFFER, text.buffer.vbo);
                        gl::EnableVertexAttribArray(0);
                        gl::VertexAttribPointer(
                            0,
                            2,
                            gl::FLOAT,
                            gl::FALSE,
                            (mem::size_of::<Vertex<RGBA>>()) as GLint,
                            0 as *const GLvoid,
                        );
                        gl::EnableVertexAttribArray(1);
                        gl::VertexAttribPointer(
                            1,
                            4,
                            gl::UNSIGNED_BYTE,
                            gl::FALSE,
                            (mem::size_of::<Vertex<RGBA>>()) as GLint,
                            (mem::size_of::<Pos>()) as *const std::ffi::c_void,
                        );

                        quads_count = text.buffer.data.data.len();
                    }
                }

                // 2 triangles, 6 vertex indices per quad
                let vertices_count = 6 * quads_count;

                gl::DrawElements(gl::TRIANGLES, vertices_count as i32, gl::UNSIGNED_SHORT, (offset * std::mem::size_of::<VertexIndex>()) as *const std::ffi::c_void);

                check();

                // next batch starts right after this one
                offset += vertices_count;
            }

            check()
        }
    }
}

/// Everything what's rendered, is quad-based, it's easier to imagine then
#[derive(Debug)]
struct Quad<T>([Vertex<T>; 4]);

/// Vertex including some primitive-specific attributes
#[derive(Debug)]
struct Vertex<T>(Pos, T);

struct Text {
    pos: Pos,
    color: RGBA,
    // TODO: should be Pos (uv for glyph coords)
    buffer: Buffer<Quad<RGBA>>
}

// Handles to primitives
pub type RectId = usize;
pub type ImageId = usize;
pub type TextId = usize;

// for indexed drawing
// raspi can do only 65k vertices in one batch
// could be configurable but it's probably better to play it safe
type VertexIndex = u16;

// one item of what is requested to be drawn
#[derive(Debug)]
pub enum DisplayItem {
    Rect(RectId),
    Image(ImageId),
    Text(TextId),
}

// what is going to be drawn, how many quads so that we know where to start with indices
// + any other params necessary to setup the pipeline (can be indirect)
#[derive(Debug)]
enum Batch {
    Rects(usize),

    Text(TextId),

    // always one quad
    // TODO: TextureId or ImageId + self.images
    Image,
}

struct Buffer<T> {
    // TODO: rename
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

    fn upload(&self) {
      if self.data.data.is_empty() {
          return;
      }

      let item_size = mem::size_of::<T>();

      // println!("upload {} x {}b", self.data.data.len(), item_size);

      unsafe {
          gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
          gl::BufferData(
            gl::ARRAY_BUFFER,
            // 4 vertices per quad
            (4 * self.data.data.len() * item_size) as isize,
            mem::transmute(&self.data.data[0]),
            gl::STATIC_DRAW
          );

          check();
      }
  }
}

impl <T: Copy> Buffer<Quad<T>> {
    fn add_quad(&mut self, a: Pos, b: Pos, data: T) -> QuadId {
        let quad = Quad([
            Vertex(a, data),
            Vertex(Pos(b.0, a.1), data),
            Vertex(Pos(a.0, b.1), data),
            Vertex(b, data),
        ]);

        self.data.add(quad)
    }

    fn set_quad_bounds(&mut self, id: QuadId, a: Pos, b: Pos) {
        let q = &mut self.data[id];

        q.0[0].0 = a;
        q.0[1].0 = Pos(b.0, a.1);
        q.0[2].0 = Pos(a.0, b.1);
        q.0[3].0 = b;
    }
}

type VboId = u32;
type QuadId = usize;


const RECT_VS: &str = r#"
  #version 100

  attribute vec2 a_pos;
  attribute vec4 a_color;

  varying vec4 v_color;

  void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_color = a_color;
  }
"#;

const RECT_FS: &str = r#"
  #version 100

  precision mediump float;

  varying vec4 v_color;

  void main() {
    gl_FragColor = v_color / 256.;
  }
"#;

// TODO:
// - sample from texture
const IMAGE_VS: &str = RECT_VS;
const IMAGE_FS: &str = RECT_FS;

// TODO:
// - translate glyphs by uniform
// - sample from texture (uv attr or glyph_index)
const TEXT_VS: &str = r#"
  #version 100

  attribute vec2 a_pos;
  attribute vec4 a_color;

  varying vec4 v_color;

  void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_color = a_color;
  }
"#;

const TEXT_FS: &str = r#"
  #version 100

  precision mediump float;

  varying vec4 v_color;

  void main() {
    gl_FragColor = v_color / 256.;
  }
"#;

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

// some store with stable ids
// TODO: freelist or something (now it just leaks memory)
//
// allocation is costy so maybe we should reuse buffers too
// (not just space for their triple but also their data buffer)
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
