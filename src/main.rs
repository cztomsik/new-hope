// basic PoC just to get some idea about perf of my iGPU & Raspis
// it is intentionally dumb & many things are missing:
// - separate opaque & alpha passes
// - Z-sorting
// - culling
// - clipping
// - basically it's just a classic painter algo for now
//
// there's also no scene, it's a kind of imgui but even without any
// event handling (nor hit-testing), no layout, no anything
//
// things missing but in a scope of this PoC:
// - text rendering & caching (now it's just a line)
// - images (no loading/decoding, just generate some checkboard)
// - round border
// - blur shadow (now it's just an outline)
// - fill round rect (with different corner radiis)
//
// the idea is to get something working on osx, raspi & in a browser
// improve it a bit and then port it back to the original project
// as a replacement for webrender

use std::ffi::CString;

use std::mem;
use std::ptr;
use std::str;

use gl::types::*;
use sdl2;
use sdl2::video::GLProfile;

fn main() {
    let sdl = sdl2::init().expect("init SDL2");
    let video = sdl.video().expect("init video");
    let mut event_pump = sdl.event_pump().expect("init event pump");

    let window = video
        .window("Test", WIDTH, 600)
        .opengl()
        .resizable()
        .build()
        .expect("init window");

    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);

    let gl_context = window.gl_create_context().expect("create context");
    gl::load_with(|name| video.gl_get_proc_address(name) as *const _);
    window.gl_make_current(&gl_context).expect("set context");

    let mut renderer = GlRenderer::new();

    loop {
        renderer.render();

        window.gl_swap_window();

        for e in event_pump.poll_iter() {
            match e {
                sdl2::event::Event::Quit { .. } => panic!("TODO: quit"),
                _ => {}
            }
        }

        ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 60));
    }
}

const WIDTH: u32 = 800;

struct GlRenderer {
    // simple pen to make it a bit more readable
    x: f32, y: f32,

    data: Vec<f32>
}

impl GlRenderer {
    fn new() -> Self {
        init();

        Self { x: 0., y: 0., data: Vec::new() }
    }

    fn render(&mut self) {
        unsafe {
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            self.x = 0.;
            self.y = 0.;

            self.demo();

            gl::BufferData(
                gl::ARRAY_BUFFER,
                (self.data.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                mem::transmute(&self.data[0]),
                gl::STATIC_DRAW,
            );

            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(
                0,
                2,
                gl::FLOAT,
                gl::FALSE,
                0 as GLint,//(mem::size_of::<f32>() * 3) as GLint,
                ptr::null(),
            );
            gl::DrawArrays(gl::TRIANGLES, 0, self.data.len() as i32);
        }
    }

    fn demo(&mut self) {
        self.navbar("Demo");
        self.h1("Create contact");

        self.focus();
        self.field("Name");
        self.field("E-mail");
        self.field("Phone");

        self.button("Create");
        self.link("Cancel");
    }

    fn navbar(&mut self, brand_text: &str) {
        self.fill_rect(0.0, 0.0, WIDTH as f32, 48.0, NAVBAR_COLOR);
        self.br();
        self.text(brand_text, NAVBAR_TEXT_COLOR);
        self.br();
        self.br();
    }

    fn h1(&mut self, text: &str) {
        self.br();
        self.text(text, TEXT_COLOR);
        self.br();
    }

    // where it is just do a round rect and don't change x/y (expects field)
    fn focus(&mut self) {
        self.shadow(self.x, self.y, INPUT_WIDTH, INPUT_HEIGHT, 0.0, 1.0, FOCUS_COLOR);
    }

    fn field(&mut self, label: &str) {
        self.br();
        self.text(label, TEXT_COLOR);
        self.br();
        self.fill_rect(self.x, self.y, INPUT_WIDTH, INPUT_HEIGHT, INPUT_COLOR);
        self.border(self.x, self.y, INPUT_WIDTH, INPUT_HEIGHT, INPUT_BORDER_COLOR);
    }

    fn button(&mut self, text: &str) {
        let w = text.len() as f32 * 10.;
        let h = 32.;

        self.fill_round(0.0, 0.0, w, h, 4., BUTTON_COLOR);
        self.x += BUTTON_PADDING;
        self.y += BUTTON_PADDING;
        self.text(text, BUTTON_TEXT_COLOR);
        self.y -= BUTTON_PADDING;
    }

    fn link(&mut self, text: &str) {
        self.text(text, LINK_COLOR);
    }

    fn br(&mut self) {
        self.x = 20.;
        self.y += 16.;
    }

    fn text(&mut self, text: &str, color: Color) {
        let w = text.len() as f32 * 10.;

        self.x += w;

        // line for now
        self.fill_rect(self.x, self.y, w, 1., color);
    }

    fn shadow(&mut self, x: f32, y: f32, w: f32, h: f32, _blur: f32, spread: f32, color: Color) {
        // solid for now
        self.fill_rect(x - spread, y - spread, w + 2. * spread, h + 2. * spread, color);
    }

    fn border(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.fill_rect(x, y, w, 1., color);
        self.fill_rect(x, y, 1., h, color);
        self.fill_rect(x, y + h - 1., w, 1., color);
        self.fill_rect(x, y + w - 1., 1., h, color);
    }

    fn fill_round(&mut self, x: f32, y: f32, w: f32, h: f32, _radius: f32, color: Color) {
        //println!("fill round {:?}", (x, y, w, h, radius, color));
        self.fill_rect(x, y, w, h, color);
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, (r, g, b): Color) {
        //println!("fill rect {:?}", (x, y, w, h, color));

        self.fill_triangle([
            (x, y, r, g, b),
            (x + w, y, r, g, b),
            (x + w, y + h, r, g, b)
        ]);

        self.fill_triangle([
            (x, y, r, g, b),
            (x + w, y + h, r, g, b),
            (x, y + h, r, g, b)
        ]);
    }

    fn fill_triangle(&mut self, data: [(f32, f32, f32, f32, f32); 3]) {
        println!("fill triangle {:?}", &data);

        for (x, y, r, g, b) in data.iter() {
            self.data.push(*x);
            self.data.push(*y);
            //self.data.push(*r);
            //self.data.push(*g);
            //self.data.push(*b);
        }
    }
}

const VERTEX_SHADER_SOURCE: &str = r#"
  #version 100

  attribute vec2 pos;

  void main() {
    // TODO: uniforms
    vec2 size = vec2(800., 600.);
    vec2 xy = (pos - (size / 2.)) / size;
    xy.y *= -1.;

    gl_Position = vec4(xy, 0.0, 1.0);
  }
"#;

const FRAGMENT_SHADER_SOURCE: &str = r#"
  #version 100

  void main() {
    gl_FragColor = vec4(1.0, 0.5, 0.2, 1.0);
  }
"#;

const INPUT_WIDTH: f32 = 100.;
const INPUT_HEIGHT: f32 = 28.;
const INPUT_COLOR: Color = (1., 1., 1.);
const _INPUT_TEXT_COLOR: Color = (0.3, 0.3, 0.3);
const INPUT_BORDER_COLOR: Color = (0.7, 0.7, 0.7);
const FOCUS_COLOR: Color = (0.7, 0.7, 1.0);

const TEXT_COLOR: Color = (0., 0., 0.);
const NAVBAR_COLOR: Color = (0.3, 0.3, 1.);
const NAVBAR_TEXT_COLOR: Color = (1., 1., 1.);
const LINK_COLOR: Color = (0., 0., 1.);

const BUTTON_COLOR: Color = (0.3, 0.3, 1.);
const BUTTON_TEXT_COLOR: Color = (1., 1., 1.);
const BUTTON_PADDING: f32 = 5.;

type Color = (f32, f32, f32);

fn init() {
    unsafe {
        // not used but webgl & opengl core profile requires it
        let mut vao = 0;
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);

        let shader_program = shader_program(VERTEX_SHADER_SOURCE, FRAGMENT_SHADER_SOURCE);
        gl::UseProgram(shader_program);

        // TODO: more buffers
        let mut vbo = 0;
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
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
