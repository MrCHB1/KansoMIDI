use std::ffi::{CString, NulError};
use std::ptr;

use gl::types::*;

pub struct Shader {
    pub id: GLuint
}

pub struct ShaderProgram {
    pub id: GLuint
}

impl Shader {
    pub unsafe fn new(source_code: &str, shader_type: GLenum) -> Result<Self, String> {
        let source_code = CString::new(source_code).unwrap();
        let shader = Self {
            id: gl::CreateShader(shader_type)
        };
        gl::ShaderSource(shader.id, 1, &source_code.as_ptr(), ptr::null());
        gl::CompileShader(shader.id);
        
        let mut success: GLint = 0;
        gl::GetShaderiv(shader.id, gl::COMPILE_STATUS, &mut success);

        if success == 1 {
            Ok(shader)
        } else {
            let mut err_log_size: GLint = 0;
            gl::GetShaderiv(shader.id, gl::INFO_LOG_LENGTH, &mut err_log_size);
            let mut err_log: Vec<u8> = Vec::with_capacity(err_log_size as usize);
            gl::GetShaderInfoLog(
                shader.id,
                err_log_size,
                &mut err_log_size,
                err_log.as_mut_ptr() as *mut _,
            );
            
            err_log.set_len(err_log_size as usize);
            let log = String::from_utf8(err_log).unwrap();
            Err(log)
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.id);
        }
    }
}

impl ShaderProgram {
    pub unsafe fn new(shaders: &[Shader]) -> Result<Self, String> {
        let program = Self {
            id: gl::CreateProgram()
        };

        for shader in shaders {
            gl::AttachShader(program.id, shader.id);
        }

        gl::LinkProgram(program.id);

        let mut success: GLint = 0;
        gl::GetProgramiv(program.id, gl::LINK_STATUS, &mut success);

        if success == 1 {
            Ok(program)
        } else {
            let mut err_log_size: GLint = 0;
            gl::GetProgramiv(program.id, gl::INFO_LOG_LENGTH, &mut err_log_size);
            let mut err_log: Vec<u8> = Vec::with_capacity(err_log_size as usize);
            gl::GetProgramInfoLog(
                program.id,
                err_log_size,
                &mut err_log_size,
                err_log.as_mut_ptr() as *mut _,
            );

            err_log.set_len(err_log_size as usize);
            let log = String::from_utf8(err_log).unwrap();
            Err(log)
        }
    }

    pub unsafe fn get_attrib_location(&self, attrib: &str) -> Result<GLuint, NulError> {
        let attrib = CString::new(attrib)?;
        Ok(gl::GetAttribLocation(self.id, attrib.as_ptr()) as GLuint)
    }

    pub fn set_float(&mut self, name: &str, value: GLfloat) -> () {
        unsafe {
            let nm: CString = CString::new(name).unwrap();
            gl::Uniform1f(
            gl::GetUniformLocation(
                self.id, 
                nm.as_ptr() as *const _
                ),
                value
            )
        }
    }

    pub fn set_uint(&mut self, name: &str, value: GLuint) -> () {
        unsafe {
            let nm: CString = CString::new(name).unwrap();
            gl::Uniform1ui(
            gl::GetUniformLocation(
                self.id, 
                nm.as_ptr() as *const _
                ),
                value
            )
        }
    }

    pub fn set_vec2(&mut self, name: &str, v0: GLfloat, v1: GLfloat) -> () {
        unsafe {
            let nm: CString = CString::new(name).unwrap();
            gl::Uniform2f(
                gl::GetUniformLocation(
                    self.id,
                    nm.as_ptr() as *const _
                ),
                v0, v1
            )
        }
    }

    pub fn set_vec3(&mut self, name: &str, v0: GLfloat, v1: GLfloat, v2: GLfloat) -> () {
        unsafe {
            let nm: CString = CString::new(name).unwrap();
            gl::Uniform3f(
                gl::GetUniformLocation(
                    self.id,
                    nm.as_ptr() as *const _
                ),
                v0, v1, v2
            )
        }
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}