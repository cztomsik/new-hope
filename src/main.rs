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
        .window("Test", 800, 600)
        .resizable()
        .build()
        .expect("init window");

    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_context_version(3, 2);

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

fn _find_sdl_gl_driver() -> Option<u32> {
    for (index, item) in sdl2::render::drivers().enumerate() {
        if item.name == "opengl" {
            return Some(index as u32);
        }
    }
    None
}

const VERTEX_SHADER_SOURCE: &str = r#"
  #version 100

  attribute vec2 pos;

  void main() {
     gl_Position = vec4(pos.x, pos.y, 0.0, 1.0);
  }
"#;

const FRAGMENT_SHADER_SOURCE: &str = r#"
  #version 100

  void main() {
    gl_FragColor = vec4(1.0, 0.5, 0.2, 1.0);
  }
"#;

struct GlRenderer {
    shader_program: u32,
    vao: u32,
}

impl GlRenderer {
    fn new() -> Self {
        let (shader_program, vao) = unsafe {
            let shader_program = shader_program(VERTEX_SHADER_SOURCE, FRAGMENT_SHADER_SOURCE);

            let vertices: [f32; 8] = [-0.5, -0.5, 0.5, -0.5, 0.0, 0.5, 1.0, 1.0];

            // gen array & buffer
            let (mut vbo, mut vao) = (0, 0);
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);

            // link them together
            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            // & load data
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                mem::transmute(&vertices[0]),
                gl::STATIC_DRAW,
            );

            // for the first attribute
            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(
                0,
                2,
                gl::FLOAT,
                gl::FALSE,
                (mem::size_of::<f32>() * 2) as GLint,
                ptr::null(),
            );

            (shader_program, vao)
        };

        GlRenderer {
            shader_program,
            vao,
        }
    }

    fn render(&mut self) {
        unsafe {
            gl::ClearColor(0.6, 0.0, 0.8, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::UseProgram(self.shader_program);
            //gl::DrawArrays(gl::TRIANGLES, 0, 3);

            let indices: [u8; 6] = [0, 1, 5, 2, 3, 5];
            let mut ibo = 0;
            gl::GenBuffers(1, &mut ibo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ibo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (indices.len() * mem::size_of::<u8>()) as GLsizeiptr,
                mem::transmute(&indices[0]),
                gl::STATIC_DRAW,
            );

            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_BYTE, ptr::null());
        }
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

// TODO: nejakou fci render(), ktera teda vykresli vsechno, co chci
// pripravit (vstupni) data pro render zhruba tak, jak opravdu budou vypadat
// pripravit to mutacni api opet zhruba tak, jak bude vypadat (a klidne to poupravit aby to nejvic sedelo tomu renderu, protoze ten bude spolu s layoutem
// zrat nejvic vykonu
