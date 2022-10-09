/// Mesh vertex tangent generator.
pub struct GenTangentContext<'a> {
    pub positions: &'a [[f32; 3]],
    pub normals: &'a [[f32; 3]],
    pub uvs: &'a [[f32; 2]],
    pub indices: &'a [u32],
    pub tangents: &'a mut [[f32; 4]],
}

impl<'a> GenTangentContext<'a> {
    #[inline(always)]
    fn base_index(&self, face: usize, vert: usize) -> usize {
        self.indices[face * 3 + vert] as usize
    }
}

impl<'a> mikktspace::Geometry for GenTangentContext<'a> {
    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.base_index(face, vert)]
    }

    fn num_faces(&self) -> usize {
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.positions[self.base_index(face, vert)]
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uvs[self.base_index(face, vert)]
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        // stick tangent back
        self.tangents[self.base_index(face, vert)] = tangent;
    }
}