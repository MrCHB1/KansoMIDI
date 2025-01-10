use gl::{self, types::*};

pub struct Buffer {
    pub id: GLuint,
    target: GLuint
}

impl Buffer {
    pub unsafe fn new(target: GLuint) -> Self {
        let mut id: GLuint = 0;
        gl::GenBuffers(1, &mut id);
        Self { id, target }
    }

    pub unsafe fn bind(&self) {
        gl::BindBuffer(self.target, self.id);
    }

    pub unsafe fn unbind(&self) {
        gl::BindBuffer(self.target, 0);
    }

    pub unsafe fn set_data<D>(&self, data: &[D], usage: GLuint) {
        self.bind();
        let (_, data_bytes, _) = data.align_to::<u8>();
        gl::BufferData(
            self.target,
            data_bytes.len() as GLsizeiptr,
            data_bytes.as_ptr() as *const _,
            usage
        );
        //gl::BindBuffer(self.target, 0);
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, [self.id].as_ptr());
        }
    }
}

pub struct VertexArray {
    pub id: GLuint
}

impl VertexArray {
    pub unsafe fn new() -> Self {
        let mut id: GLuint = 0;
        gl::GenVertexArrays(1, &mut id);
        Self { id }
    }

    pub unsafe fn bind(&self) {
        gl::BindVertexArray(self.id);
    }

    pub unsafe fn unbind(&self) {
        gl::BindVertexArray(0);
    }

    pub unsafe fn set_attribute<V: Sized>(
        &self,
        type_: u32,
        attrib_pos: GLuint,
        components: GLint,
        offset: GLint
    ) {
        self.bind();
        gl::VertexAttribPointer(
            attrib_pos,
            components,
            type_,
            gl::FALSE,
            std::mem::size_of::<V>() as GLint,
            offset as *const _
        );
        gl::EnableVertexAttribArray(attrib_pos);
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, [self.id].as_ptr());
        }
    }
}

#[macro_export]
macro_rules! set_attribute {
    ($type_:ident :: $tfield:tt, $vbo:ident, $pos:tt, $t:ident :: $field:tt) => {{
        let dummy = core::mem::MaybeUninit::<$t>::uninit();
        let dummy_ptr = dummy.as_ptr();
        let member_ptr = core::ptr::addr_of!((*dummy_ptr).$field);
        const fn size_of_raw<T>(_: *const T) -> usize {
            core::mem::size_of::<T>()
        }
        let member_offset = member_ptr as i32 - dummy_ptr as i32;
        $vbo.set_attribute::<$t>(
            $type_::$tfield,
            $pos,
            (size_of_raw(member_ptr) / 4) as i32,
            member_offset,
        )
    }};
}