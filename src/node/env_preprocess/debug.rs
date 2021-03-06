use genmesh::{
    generators::{IndexedPolygon, SharedVertex},
    Triangulate,
};

use rendy::{
    command::{QueueId, RenderPassEncoder},
    factory::Factory,
    graph::{render::*, GraphContext, NodeBuffer, NodeImage},
    hal::{device::Device, pso::DescriptorPool},
    memory::MemoryUsageValue,
    mesh::{AsVertex, Mesh, Position},
    resource::{Buffer, BufferInfo, DescriptorSetLayout, Escape, Handle},
    shader::{PathBufShaderInfo, ShaderKind, SourceLanguage},
};

use rendy::hal;

use crate::node::env_preprocess::Aux;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct UniformArgs {
    proj: nalgebra::Matrix4<f32>,
    view: nalgebra::Matrix4<f32>,
}

lazy_static::lazy_static! {
    static ref VERTEX: PathBufShaderInfo = PathBufShaderInfo::new(
        std::path::PathBuf::from(crate::application_root_dir()).join("assets/shaders/debug.vert"),
        ShaderKind::Vertex,
        SourceLanguage::GLSL,
        "main",
    );

    static ref FRAGMENT: PathBufShaderInfo = PathBufShaderInfo::new(
        std::path::PathBuf::from(crate::application_root_dir()).join("assets/shaders/debug.frag"),
        ShaderKind::Fragment,
        SourceLanguage::GLSL,
        "main",
    );

    static ref SHADERS: rendy::shader::ShaderSetBuilder = rendy::shader::ShaderSetBuilder::default()
        .with_vertex(&*VERTEX).unwrap()
        .with_fragment(&*FRAGMENT).unwrap();
}

#[derive(Debug, PartialEq, Eq)]
pub struct Settings {
    align: u64,
}

impl<B: hal::Backend> From<&Aux<B>> for Settings {
    fn from(aux: &Aux<B>) -> Self {
        Self::from_aux(aux)
    }
}

impl<B: hal::Backend> From<&mut Aux<B>> for Settings {
    fn from(aux: &mut Aux<B>) -> Self {
        Self::from_aux(aux)
    }
}

impl Settings {
    const UNIFORM_SIZE: u64 = std::mem::size_of::<UniformArgs>() as u64;

    fn from_aux<B: hal::Backend>(aux: &Aux<B>) -> Self {
        Settings { align: aux.align }
    }

    #[inline]
    fn buffer_frame_size(&self) -> u64 {
        ((Self::UNIFORM_SIZE - 1) / self.align + 1) * self.align
    }
}

#[derive(Debug, Default)]
pub struct PipelineDesc;

pub struct Pipeline<B: hal::Backend> {
    cube: Mesh<B>,
    set: B::DescriptorSet,
    pool: B::DescriptorPool,
    #[allow(dead_code)]
    buffer: Escape<Buffer<B>>,
}

impl<B: hal::Backend> std::fmt::Debug for Pipeline<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Equirect Pipeline")
    }
}

impl<B> SimpleGraphicsPipelineDesc<B, Aux<B>> for PipelineDesc
where
    B: hal::Backend,
{
    type Pipeline = Pipeline<B>;

    fn vertices(
        &self,
    ) -> Vec<(
        Vec<hal::pso::Element<hal::format::Format>>,
        hal::pso::ElemStride,
        hal::pso::VertexInputRate,
    )> {
        vec![Position::vertex().gfx_vertex_input_desc(hal::pso::VertexInputRate::Vertex)]
    }

    fn colors(&self) -> Vec<hal::pso::ColorBlendDesc> {
        vec![hal::pso::ColorBlendDesc {
            mask: hal::pso::ColorMask::ALL,
            blend: None,
        }]
    }

    fn depth_stencil(&self) -> Option<hal::pso::DepthStencilDesc> {
        None
    }

    fn load_shader_set(
        &self,
        factory: &mut Factory<B>,
        _aux: &Aux<B>,
    ) -> rendy::shader::ShaderSet<B> {
        SHADERS.build(factory, Default::default()).unwrap()
    }

    fn layout(&self) -> Layout {
        Layout {
            sets: vec![SetLayout {
                bindings: vec![
                    hal::pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: hal::pso::DescriptorType::UniformBuffer,
                        count: 1,
                        stage_flags: hal::pso::ShaderStageFlags::GRAPHICS,
                        immutable_samplers: false,
                    },
                    hal::pso::DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: hal::pso::DescriptorType::Sampler,
                        count: 1,
                        stage_flags: hal::pso::ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                    hal::pso::DescriptorSetLayoutBinding {
                        binding: 2,
                        ty: hal::pso::DescriptorType::SampledImage,
                        count: 1,
                        stage_flags: hal::pso::ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                ],
            }],
            push_constants: Vec::new(),
        }
    }

    fn build<'a>(
        self,
        _ctx: &GraphContext<B>,
        factory: &mut Factory<B>,
        queue: QueueId,
        aux: &Aux<B>,
        buffers: Vec<NodeBuffer>,
        images: Vec<NodeImage>,
        set_layouts: &[Handle<DescriptorSetLayout<B>>],
    ) -> Result<Pipeline<B>, hal::pso::CreationError> {
        assert!(buffers.is_empty());
        assert!(images.is_empty());
        assert!(set_layouts.len() == 1);

        let cube = genmesh::generators::Cube::new();
        let cube_vertices: Vec<_> = cube
            .shared_vertex_iter()
            .map(|v| Position(v.pos.into()))
            .collect();

        let cube_flattened_vertices: Vec<_> =
            genmesh::Vertices::vertices(cube.indexed_polygon_iter().triangulate())
                .map(|i| cube_vertices[i])
                .collect();

        let cube = Mesh::<B>::builder()
            .with_vertices(&cube_flattened_vertices[..])
            .build(queue, factory)
            .unwrap();

        let mut pool = unsafe {
            factory.create_descriptor_pool(
                1,
                vec![
                    hal::pso::DescriptorRangeDesc {
                        ty: hal::pso::DescriptorType::UniformBuffer,
                        count: 1,
                    },
                    hal::pso::DescriptorRangeDesc {
                        ty: hal::pso::DescriptorType::Sampler,
                        count: 1,
                    },
                    hal::pso::DescriptorRangeDesc {
                        ty: hal::pso::DescriptorType::SampledImage,
                        count: 1,
                    },
                ],
                hal::pso::DescriptorPoolCreateFlags::empty(),
            )?
        };

        let settings: Settings = aux.into();

        let mut buffer = factory
            .create_buffer(
                BufferInfo {
                    size: settings.buffer_frame_size(),
                    usage: hal::buffer::Usage::UNIFORM,
                },
                MemoryUsageValue::Dynamic,
            )
            .unwrap();

        let set = unsafe {
            let set = pool.allocate_set(&set_layouts[0].raw()).unwrap();
            factory.write_descriptor_sets(vec![
                hal::pso::DescriptorSetWrite {
                    set: &set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: Some(hal::pso::Descriptor::Buffer(
                        buffer.raw(),
                        Some(0)..Some(Settings::UNIFORM_SIZE),
                    )),
                },
                hal::pso::DescriptorSetWrite {
                    set: &set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: Some(hal::pso::Descriptor::Sampler(
                        aux.spec_cubemap.as_ref().unwrap().sampler().raw(),
                    )),
                },
                hal::pso::DescriptorSetWrite {
                    set: &set,
                    binding: 2,
                    array_offset: 0,
                    descriptors: Some(hal::pso::Descriptor::Image(
                        aux.spec_cubemap.as_ref().unwrap().view().raw(),
                        hal::image::Layout::ShaderReadOnlyOptimal,
                    )),
                },
            ]);
            set
        };

        let origin = nalgebra::Point3::origin();
        unsafe {
            factory
                .upload_visible_buffer(
                    &mut buffer,
                    0,
                    &[UniformArgs {
                        proj: {
                            let mut proj = nalgebra::Perspective3::<f32>::new(
                                1.0,
                                std::f32::consts::FRAC_PI_2,
                                0.1,
                                100.0,
                            )
                            .to_homogeneous();
                            proj[(1, 1)] *= -1.0;
                            proj
                        },
                        view: nalgebra::Matrix4::look_at_rh(
                            &origin,
                            &nalgebra::Point3::new(0.0, 0.0, 1.0),
                            &-nalgebra::Vector3::y(),
                        ),
                    }],
                )
                .unwrap()
        };

        Ok(Pipeline {
            cube,
            set,
            pool,
            buffer,
        })
    }
}

impl<B> SimpleGraphicsPipeline<B, Aux<B>> for Pipeline<B>
where
    B: hal::Backend,
{
    type Desc = PipelineDesc;

    fn prepare(
        &mut self,
        _factory: &Factory<B>,
        _queue: QueueId,
        _set_layouts: &[Handle<DescriptorSetLayout<B>>],
        _index: usize,
        _aux: &Aux<B>,
    ) -> PrepareResult {
        PrepareResult::DrawReuse
    }

    fn draw(
        &mut self,
        layout: &B::PipelineLayout,
        mut encoder: RenderPassEncoder<'_, B>,
        _index: usize,
        _aux: &Aux<B>,
    ) {
        assert!(self
            .cube
            .bind(0, &[Position::vertex()], &mut encoder)
            .is_ok());
        unsafe {
            encoder.bind_graphics_descriptor_sets(layout, 0, Some(&self.set), std::iter::empty());
            encoder.draw(0..36, 0..6);
        }
    }

    fn dispose(mut self, factory: &mut Factory<B>, _aux: &Aux<B>) {
        unsafe {
            self.pool.reset();
            factory.destroy_descriptor_pool(self.pool);
        }
    }
}
