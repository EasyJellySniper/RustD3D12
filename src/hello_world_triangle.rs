// HelloWorldTriangle.rs - To implement the hello world triangle rendering

use std::mem::ManuallyDrop;
use std::u32;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows_core::Interface;
use windows::Win32::Graphics::Direct3D::Fxc::*;
use windows::core::*;
use std::path::PathBuf;
use std::fs;
use std::time::*;

static mut GOVERLAY_STATE : Option<ID3D12PipelineState> = None;
static mut GHELLO_ROOT_SIGNATURE : Option<ID3D12RootSignature> = None;
static mut GSTART_TIME : Option<SystemTime> = None;

use crate::graphic_device;

const fn decode_utf8_char(bytes: &[u8], mut pos: usize) -> Option<(u32, usize)> {
    if bytes.len() == pos {
        return None;
    }
    let ch = bytes[pos] as u32;
    pos += 1;
    if ch <= 0x7f {
        return Some((ch, pos));
    }
    if (ch & 0xe0) == 0xc0 {
        if bytes.len() - pos < 1 {
            return None;
        }
        let ch2 = bytes[pos] as u32;
        pos += 1;
        if (ch2 & 0xc0) != 0x80 {
            return None;
        }
        let result: u32 = ((ch & 0x1f) << 6) | (ch2 & 0x3f);
        if result <= 0x7f {
            return None;
        }
        return Some((result, pos));
    }
    if (ch & 0xf0) == 0xe0 {
        if bytes.len() - pos < 2 {
            return None;
        }
        let ch2 = bytes[pos] as u32;
        pos += 1;
        let ch3 = bytes[pos] as u32;
        pos += 1;
        if (ch2 & 0xc0) != 0x80 || (ch3 & 0xc0) != 0x80 {
            return None;
        }
        let result = ((ch & 0x0f) << 12) | ((ch2 & 0x3f) << 6) | (ch3 & 0x3f);
        if result <= 0x7ff || (0xd800 <= result && result <= 0xdfff) {
            return None;
        }
        return Some((result, pos));
    }
    if (ch & 0xf8) == 0xf0 {
        if bytes.len() - pos < 3 {
            return None;
        }
        let ch2 = bytes[pos] as u32;
        pos += 1;
        let ch3 = bytes[pos] as u32;
        pos += 1;
        let ch4 = bytes[pos] as u32;
        pos += 1;
        if (ch2 & 0xc0) != 0x80 || (ch3 & 0xc0) != 0x80 || (ch4 & 0xc0) != 0x80 {
            return None;
        }
        let result =
            ((ch & 0x07) << 18) | ((ch2 & 0x3f) << 12) | ((ch3 & 0x3f) << 6) | (ch4 & 0x3f);
        if result <= 0xffff || 0x10ffff < result {
            return None;
        }
        return Some((result, pos));
    }
    None
}

fn string_to_pcwstr(s : String) -> PCWSTR
{
    let input: &[u8] = s.as_bytes();
    // gives a buffer size that is big enough for the input string.
    // as non-literal implementaion does not allow const define for input.
    const OUTPUT_LEN: usize = 65536;
    let output: &[u16; OUTPUT_LEN] = {
        let mut buffer = [0; OUTPUT_LEN];
        let mut input_pos = 0;
        let mut output_pos = 0;
        while let Some((mut code_point, new_pos)) = decode_utf8_char(input, input_pos) 
        {
            input_pos = new_pos;
            if code_point <= 0xffff 
            {
                buffer[output_pos] = code_point as u16;
                output_pos += 1;
            } 
            else 
            {
                code_point -= 0x10000;
                buffer[output_pos] = 0xd800 + (code_point >> 10) as u16;
                output_pos += 1;
                buffer[output_pos] = 0xdc00 + (code_point & 0x3ff) as u16;
                output_pos += 1;
            }
        }
        &{ buffer }
    };
    return PCWSTR::from_raw(output.as_ptr());
}

// function to create pipeline for hello world triangle
pub fn create_pipeline()
{
    unsafe 
    {
        let device = graphic_device::get_device();

        // create a root signature with a pixel-only 32-bit constant.
        let root_parameter_constant = D3D12_ROOT_PARAMETER
        {
            ParameterType : D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous : D3D12_ROOT_PARAMETER_0
            {
                // nested initializor for union sturcture
                Constants : D3D12_ROOT_CONSTANTS
                {
                    ShaderRegister : 0,
                    RegisterSpace : 0,
                    Num32BitValues : 1,
                }
            },
            ShaderVisibility : D3D12_SHADER_VISIBILITY_PIXEL,
            ..D3D12_ROOT_PARAMETER::default()
        };

        let root_signature_desc = D3D12_ROOT_SIGNATURE_DESC
        {
            NumParameters : 1,
            pParameters : &root_parameter_constant,
            ..D3D12_ROOT_SIGNATURE_DESC::default()
        };
        let mut root_signature_blob : Option<ID3DBlob> = None;

        let _ = D3D12SerializeRootSignature(&root_signature_desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut root_signature_blob, None);
        // convert the ID3DBlob::GetBufferPointer() to *const u8 with std::slice::fromw_raw_parts()
        let root_blob_data = std::slice::from_raw_parts(root_signature_blob.as_ref().unwrap().GetBufferPointer() as *const u8, root_signature_blob.as_ref().unwrap().GetBufferSize());
        if let Ok(x) = device.CreateRootSignature::<ID3D12RootSignature>(0, root_blob_data)
        {
            GHELLO_ROOT_SIGNATURE = Some(x);
        }

        if GHELLO_ROOT_SIGNATURE.is_none()
        {
            println!("Error during root signature creation!");
            return;
        }

        // compile shaders with D3DCompileFromFile just for demo purpose, as it uses old FXC compiler
        // in real world application, you might want to use DirectXShaderCompiler binary for 6.0 shader models and above
        // fs::canonicalize() to get absolute path
        // PathBuf::from() to establish a path structure
        let shader_file_name = string_to_pcwstr(fs::canonicalize(PathBuf::from("./shaders/hello_world_triangle.hlsl")).unwrap().display().to_string());
        let compile_flag = D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION;
        let mut vs_blob : Option<ID3DBlob> = None;
        let mut ps_blob : Option<ID3DBlob> = None;
        
        // if compile error message is needed, setup ID3DBlob for the last parameter as well, I skip it for now
        let _ = D3DCompileFromFile(shader_file_name, None, None, s!("HelloWorldVS"), s!("vs_5_1"), compile_flag, 0, &mut vs_blob, None);
        let _ = D3DCompileFromFile(shader_file_name, None, None, s!("HelloWorldPS"), s!("ps_5_1"), compile_flag, 0, &mut ps_blob, None);
        if vs_blob.is_none() || ps_blob.is_none()
        {
            println!("Error during shader creation!");
            return;
        }

        // setup byte code structure
        let vs_bytecode = D3D12_SHADER_BYTECODE
        {
            pShaderBytecode : vs_blob.as_ref().unwrap().GetBufferPointer(),
            BytecodeLength : vs_blob.as_ref().unwrap().GetBufferSize(),
        };

        let ps_bytecode = D3D12_SHADER_BYTECODE
        {
            pShaderBytecode : ps_blob.as_ref().unwrap().GetBufferPointer(),
            BytecodeLength : ps_blob.as_ref().unwrap().GetBufferSize(),
        };

        // setup an overlay rasterizer
        let mut overlay_rasterize_state = D3D12_RASTERIZER_DESC::default();
        overlay_rasterize_state = D3D12_RASTERIZER_DESC
        {
            FillMode : D3D12_FILL_MODE_SOLID,
            CullMode : D3D12_CULL_MODE_NONE,
            ..overlay_rasterize_state
        };

        // setup color write mask for render target
        let render_target_blend_desc = D3D12_RENDER_TARGET_BLEND_DESC
        {
            RenderTargetWriteMask : D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
            ..D3D12_RENDER_TARGET_BLEND_DESC::default()
        };

        // setup RTV format array, unused slot must be DXGI_UNKNOWN
        let mut rtv_format_list = [DXGI_FORMAT_UNKNOWN; 8];
        rtv_format_list[0] = graphic_device::get_back_buffer_format();

        // create pipeline state
        let pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC
        {
            // pRootSignature somehow implemented as ManuallyDrop, just setup one for it
            pRootSignature : ManuallyDrop::new(GHELLO_ROOT_SIGNATURE.clone()),
            VS : vs_bytecode,
            PS : ps_bytecode,
            RasterizerState : overlay_rasterize_state,
            BlendState : D3D12_BLEND_DESC
            {
                RenderTarget : [render_target_blend_desc; 8],
                ..D3D12_BLEND_DESC::default()
            },
            DepthStencilState : D3D12_DEPTH_STENCIL_DESC::default(),
            SampleMask : u32::MAX,
            PrimitiveTopologyType : D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets : 1,
            RTVFormats : rtv_format_list,
            SampleDesc : DXGI_SAMPLE_DESC
            {
                Count : 1,             
                Quality : 0,  
            },
            ..D3D12_GRAPHICS_PIPELINE_STATE_DESC::default()
        };

        if let Ok(x) = device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&pso_desc)
        {
            GOVERLAY_STATE = Some(x);
        }

        if GOVERLAY_STATE.is_none()
        {
            println!("Error during pipeline state creation!");
        }

        // store the start time
        GSTART_TIME = Some(std::time::SystemTime::now());
    }
}

// function to render for hello world triangle
pub fn render(width : u32, height : u32)
{
    unsafe 
    {
        // reset command buffers
        let command_allocator = graphic_device::get_command_allocator();
        let command_list = graphic_device::get_command_list();
        let _ = command_allocator.Reset();
        let _ = command_list.Reset(command_allocator, None);

        // transition and clear backbuffer
        let back_buffer_handle = graphic_device::get_back_buffer_rtv();
        let clear_color : [f32; 4] = [0.0, 0.2, 0.4, 1.0 ];

        // D3D12_RESOURCE_TRANSITION_BARRIER desc
        let rtv_transition_barrier = D3D12_RESOURCE_TRANSITION_BARRIER
        {
            pResource : ManuallyDrop::new(graphic_device::get_back_buffer_resource().clone()),
            StateBefore : D3D12_RESOURCE_STATE_PRESENT,
            StateAfter : D3D12_RESOURCE_STATE_RENDER_TARGET,
            Subresource : 0,
        };

        // D3D12_RESOURCE_BARRIER desc
        let rtv_resource_barrier = D3D12_RESOURCE_BARRIER
        {
            Type : D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Anonymous : D3D12_RESOURCE_BARRIER_0
            {
                Transition : ManuallyDrop::new(rtv_transition_barrier),
            },
            ..D3D12_RESOURCE_BARRIER::default()
        };
        command_list.ResourceBarrier(&[rtv_resource_barrier; 1]);
        
        command_list.OMSetRenderTargets(1, Some(&back_buffer_handle), FALSE, None);
        command_list.ClearRenderTargetView(back_buffer_handle, &clear_color, None);

        // bind graphic state, root signature, viewport and scissor rect
        command_list.SetPipelineState(GOVERLAY_STATE.as_ref().unwrap());
        command_list.SetGraphicsRootSignature(GHELLO_ROOT_SIGNATURE.as_ref().unwrap());

        let viewport_desc = D3D12_VIEWPORT
        {
            Width : width as f32,
            Height : height as f32,
            MinDepth : 0.0,
            MaxDepth : 1.0,
            TopLeftX : 0.0,
            TopLeftY : 0.0,
        };
        command_list.RSSetViewports(&[viewport_desc; 1]);

        let scissor_desc = RECT
        {
            left : 0,
            top : 0,
            right : width as i32,
            bottom : height as i32,
        };
        command_list.RSSetScissorRects(&[scissor_desc; 1]);

        // set constant number as elapsed time
        let start_time = GSTART_TIME.as_ref().unwrap();
        let current_time = SystemTime::now();
        if let Ok(x) = current_time.duration_since(*start_time)
        {
            command_list.SetGraphicsRoot32BitConstant(0,x.as_millis() as u32,0);
        }

        // set topology and draw full screen quad
        command_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        command_list.DrawInstanced(6, 1, 0, 0);

        // transition back buffer to present state
        let present_transition_barrier = D3D12_RESOURCE_TRANSITION_BARRIER
        {
            pResource : ManuallyDrop::new(graphic_device::get_back_buffer_resource().clone()),
            StateBefore : D3D12_RESOURCE_STATE_RENDER_TARGET,
            StateAfter : D3D12_RESOURCE_STATE_PRESENT,
            Subresource : 0,
        };

        let present_resource_barrier = D3D12_RESOURCE_BARRIER
        {
            Type : D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Anonymous : D3D12_RESOURCE_BARRIER_0
            {
                Transition : ManuallyDrop::new(present_transition_barrier),
            },
            ..D3D12_RESOURCE_BARRIER::default()
        };
        command_list.ResourceBarrier(&[present_resource_barrier; 1]);

        // close command list and execute
        let _ = command_list.Close();
        graphic_device::get_command_queue().ExecuteCommandLists(&[Some(command_list.cast().unwrap())]);
    }
}