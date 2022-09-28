#[derive(Default)]
pub struct TreeByteBufferNode {
    pub patch_addr: usize,
    pub buffer: TreeByteBuffer,
}

impl TreeByteBufferNode {
    pub fn new() -> Self {
        Default::default()
    }
}

#[derive(Default)]
pub struct TreeByteBuffer {
    pub bytes: Vec<u8>,
    pub childs: Vec<TreeByteBufferNode>,
    section_idx: Option<usize>,
}

#[derive(Default, Clone)]
struct ByteSectionPatch {
    patch_addr: usize,
    section_idx: usize,
}

#[derive(Default, Clone)]
struct ByteSection {
    bytes: Vec<u8>,
    patches: Vec<ByteSectionPatch>,
}

impl TreeByteBuffer {
    pub fn new() -> Self {
        Default::default()
    }

    fn allocate_section_idx(&mut self) -> usize {
        let mut section_idx = 0;
        self.section_idx = Some(section_idx);
        section_idx += 1;
        
        for child in &mut self.childs {
            Self::allocate_section_idx_inner(child, &mut section_idx);
        }

        section_idx + 1
    }

    fn allocate_section_idx_inner(node: &mut TreeByteBufferNode, section_idx: &mut usize) {
        let buffer = &mut node.buffer;
        buffer.section_idx = Some(*section_idx);
        *section_idx += 1;

        for child in &mut buffer.childs {
            Self::allocate_section_idx_inner(child, section_idx);
        }
    }

    pub fn write_packed(mut self, writer: &mut impl std::io::Write) {
        let section_count = self.allocate_section_idx();

        let mut sections = vec![Default::default(); section_count];

        let mut tree_nodes = vec![self];
        // flatten all tree nodes into flat sections
        while !tree_nodes.is_empty() {
            let mut next_nodes = vec![];

            for node in tree_nodes {
                let TreeByteBuffer { bytes, childs, section_idx } = node;

                let sec_idx = section_idx.unwrap();
                let section = sections.get_mut(sec_idx).unwrap();

                *section = ByteSection {
                    bytes: bytes,
                    patches: childs.iter()
                        .map(|child| ByteSectionPatch {
                            patch_addr: child.patch_addr,
                            section_idx: child.buffer.section_idx.unwrap(),
                        })
                        .collect(),
                };

                for child in childs {
                    next_nodes.push(child.buffer);
                }
            }

            tree_nodes = next_nodes;
        }

        let mut base_address = 0;
        // populate section base addresses for patching offset address
        let sections_base_addr = sections.iter()
            .map(|section| {
                let current_addr = base_address;
                base_address += section.bytes.len();
                current_addr
            })
            .collect::<Vec<_>>();

        // patch offset address
        for (section, section_base_addr) in sections.iter_mut().zip(sections_base_addr.iter()) {
            for patch in section.patches.iter_mut() {
                let absolute_patch_addr = section_base_addr + patch.patch_addr;
                let target_section_base_addr = sections_base_addr[patch.section_idx];

                let offset = (target_section_base_addr - absolute_patch_addr) as u64;

                // patch the offset
                section.bytes[patch.patch_addr..patch.patch_addr + 8]
                    .copy_from_slice(&offset.to_ne_bytes());
            }
        }

        // write the final binaries
        for section in sections {
            writer.write_all(section.bytes.as_slice()).unwrap();
        }
    }
}