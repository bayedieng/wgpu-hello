use std::borrow::Cow;
use std::mem;
use std::slice;
use wgpu::util::DeviceExt;

async fn run() {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        )
        .await
        .unwrap();

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../double.wgsl"))),
    });

    let input_data: &[f32] = &[4., 6.];
    let input_raw: &[u8] = unsafe {
        slice::from_raw_parts(
            input_data.as_ptr() as *const u8,
            input_data.len() * mem::size_of::<f32>(),
        )
    };

    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: input_raw,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });
    let size = mem::size_of_val(input_raw) as u64;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output buffer"),
        size: size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader_module,
        entry_point: "main",
    });

    let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: input_buffer.as_entire_binding(),
        }],
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("double the inputs");
        cpass.dispatch_workgroups(input_data.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&input_buffer, 0, &output_buffer, 0, size);
    queue.submit(Some(encoder.finish()));

    let buffer_slice = output_buffer.slice(..);
    let (sender, receiver) = flume::bounded(1);
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
    device.poll(wgpu::Maintain::Wait);

    if let Ok(Ok(())) = receiver.recv_async().await {
        let data = buffer_slice.get_mapped_range();
        let result =
            unsafe { slice::from_raw_parts(data.as_ptr() as *const f32, input_data.len()) }
                .to_vec();
        drop(data);
        output_buffer.unmap();
        println!("{result:?}")
    }
}
fn main() {
    pollster::block_on(run());
}
