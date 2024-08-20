// graphic_device.rs - The file to implement D3D12 stuff

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Dxgi::*;
use windows_core::PCSTR;
use Common::*;
use windows::Win32::Foundation::*;
use windows_core::Interface;
use windows::Win32::System::Threading::*;
use std::mem;
use std::ffi::c_void;
use libc;

// global D3D12 interfaces
static mut GDXGI_FACTORY : Option<IDXGIFactory4> = None;
static mut GD3D12_DEVICE : Option<ID3D12Device> = None;
static mut GMAIN_COMMAND_QUEUE : Option<ID3D12CommandQueue> = None;
static mut GMAIN_COMMAND_ALLOCATOR : Option<ID3D12CommandAllocator> = None;
static mut GMAIN_COMMAND_LIST : Option<ID3D12GraphicsCommandList> = None;
static mut GDEBUG_INFO_QUEUE : Option<ID3D12InfoQueue> = None;

const GMAXFRAME : usize = 2;
static GBACK_BUFFER_FORMAT : DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;
static mut GSUPPORT_SCREEN_TEARING : bool = false;
static mut GSWAPCHAIN : Option<IDXGISwapChain3> = None;
static mut GSWAPCHAIN_HEAP : Option<ID3D12DescriptorHeap> = None;
static mut GRTV_DESCRIPTOR_SIZE : u32 = 0;
static mut GSWAPCHAIN_RESOURCE : [Option<ID3D12Resource>; GMAXFRAME] = [None,None];
static mut GCURRENT_FRAME_INDEX : u32 = 0;

static mut GMAIN_FENCE_VALUE : u64 = 0;
static mut GMAIN_FENCE : Option<ID3D12Fence> = None;
static mut GMAIN_FENCE_EVENT : Option<HANDLE> = None;

// function to create device
fn create_device()
{
    unsafe 
    {
        let mut dxgi_factory_flag : DXGI_CREATE_FACTORY_FLAGS = DXGI_CREATE_FACTORY_FLAGS::default();

        // enable debug layer
        let mut debug_controller : Option<ID3D12Debug> = None;
        if let Ok(()) = D3D12GetDebugInterface(&mut debug_controller)
        {
            debug_controller.as_ref().unwrap().EnableDebugLayer();
            dxgi_factory_flag = dxgi_factory_flag | DXGI_CREATE_FACTORY_DEBUG;
        }

        // create DXGI factory
        if let Ok(x) = CreateDXGIFactory2::<IDXGIFactory4>(dxgi_factory_flag)
        {
            GDXGI_FACTORY = Some(x);
        }

        // create d3d device after dxgi factory is created
        let mut d3d12_device : Option<ID3D12Device> = None;
        if GDXGI_FACTORY.is_some()
        {
            // try adapters from the highest feature level to lowest
            let feature_levels : [D3D_FEATURE_LEVEL; 3] = [D3D_FEATURE_LEVEL_12_2, D3D_FEATURE_LEVEL_12_1, D3D_FEATURE_LEVEL_12_0];
            let feature_levels_name : [&str;3] = ["12_2","12_1","12_0"];
            let mut feature_index = 0;
            let mut adapter_index;

            'FeatureLevelLoop : loop
            {
                adapter_index = 0;
                loop
                {
                    if let Ok(x) = GDXGI_FACTORY.as_ref().unwrap().EnumAdapters1(adapter_index)
                    {
                        let adapter_desc = x.GetDesc1().unwrap();
                        if (adapter_desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) > 0
                        {
                            // skip software adapter
                            adapter_index = adapter_index + 1;
                            continue;
                        }

                        // whenever an adapter with the highest feature level succeeds a intialization, jump out
                        if let Ok(_) = D3D12CreateDevice(&x, feature_levels[feature_index], &mut d3d12_device)
                        {
                            println!("Selected adapter for D3D12CreateDevice: {}", String::from_utf16(&adapter_desc.Description).unwrap());
                            println!("Intialized with feature level: {}", feature_levels_name[feature_index]);
                            GD3D12_DEVICE = d3d12_device;
                            break 'FeatureLevelLoop;
                        }
                    }
                    else
                    {
                        // jump out when EnumAdapters1 stops returning anything.
                        break;    
                    }

                    adapter_index = adapter_index + 1;
                }

                feature_index = feature_index + 1;
            }
        }

        // cache an ID3D12InfoQueue interface for use if device creation and debug layer are ready.
        // since the visual studio code failed to catch the D3D error output, I'm going to print them out manually.
        if GD3D12_DEVICE.is_some() && debug_controller.is_some()
        {
            GDEBUG_INFO_QUEUE = Some(GD3D12_DEVICE.as_ref().unwrap().cast().unwrap());
        }
    }
}

// function to create command buffers, which includes the queue, allocator and list
fn create_command_buffers()
{
    unsafe
    {
        let device = GD3D12_DEVICE.as_ref().unwrap();

        // create queue
        let queue_desc = D3D12_COMMAND_QUEUE_DESC
        {
            Type : D3D12_COMMAND_LIST_TYPE_DIRECT,
            Flags : D3D12_COMMAND_QUEUE_FLAG_NONE,
            ..D3D12_COMMAND_QUEUE_DESC::default()
        };

        if let Ok(x) = device.CreateCommandQueue::<ID3D12CommandQueue>(&queue_desc)
        {
            GMAIN_COMMAND_QUEUE = Some(x);
        }

        // create allocator
        if let Ok(x) = device.CreateCommandAllocator::<ID3D12CommandAllocator>(D3D12_COMMAND_LIST_TYPE_DIRECT)
        {
            GMAIN_COMMAND_ALLOCATOR = Some(x);
        }

        // create list
        if let Ok(x) = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, GMAIN_COMMAND_ALLOCATOR.as_ref(), None)
        {
            GMAIN_COMMAND_LIST = Some(x);
            // close the command list at the beginning as the render loop will reset it.
            let _ = GMAIN_COMMAND_LIST.as_ref().unwrap().Close();
        }
    }
}

// function to create swapchain
fn create_swapchain(h_wnd : HWND, render_width : u32, render_height : u32)
{
    unsafe
    {
        let device = GD3D12_DEVICE.as_ref().unwrap();
        // use the cast() function in windows_core::Interface, it's basically the equivalent of QueryInterface in c++ COM
        let factory : IDXGIFactory5 = GDXGI_FACTORY.as_ref().unwrap().cast().unwrap();

        // check screen tearing support, with ALLOW_TEARING we can render above monitor's refresh rate
        let mut support_tearing : bool = false;
        // to force-convert as c_void pointer, make it as _ first then the c_void
        let _ = factory.CheckFeatureSupport(DXGI_FEATURE_PRESENT_ALLOW_TEARING, &mut support_tearing as *mut _ as *mut c_void
            , mem::size_of::<DXGI_FEATURE>().try_into().unwrap());
        GSUPPORT_SCREEN_TEARING = support_tearing;

        let mut swapchain_flags : u32 = 0;
        if GSUPPORT_SCREEN_TEARING
        {
            swapchain_flags = swapchain_flags | DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 as u32;
        }

        // create swapchain
        let swapchain_desc = DXGI_SWAP_CHAIN_DESC1
        {
            BufferCount : GMAXFRAME as u32,
            Width : render_width,
            Height : render_height,
            Format : GBACK_BUFFER_FORMAT,
            BufferUsage : DXGI_USAGE_RENDER_TARGET_OUTPUT,
            SwapEffect : DXGI_SWAP_EFFECT_FLIP_DISCARD,
            SampleDesc : DXGI_SAMPLE_DESC
            {
                Count : 1,
                Quality : 0,
            },
            Flags : swapchain_flags,
            ..DXGI_SWAP_CHAIN_DESC1::default()
        };

        if let Ok(x) = GDXGI_FACTORY.as_ref().unwrap().CreateSwapChainForHwnd(GMAIN_COMMAND_QUEUE.as_ref().unwrap(), h_wnd, &swapchain_desc, None, None)
        {
            GSWAPCHAIN = Some(x.cast().unwrap());
        }

        // disable alt+enter behavior for now
        let _ = GDXGI_FACTORY.as_ref().unwrap().MakeWindowAssociation(h_wnd, DXGI_MWA_NO_ALT_ENTER);

        // create swapchain descriptor heap
        let swapchain_descriptor_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC
        {
            NumDescriptors : GMAXFRAME as u32,
            Type : D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            Flags : D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            ..D3D12_DESCRIPTOR_HEAP_DESC::default()
        };

        if let Ok(x) = device.CreateDescriptorHeap::<ID3D12DescriptorHeap>(&swapchain_descriptor_heap_desc)
        {
            GSWAPCHAIN_HEAP = Some(x);
        }
        GRTV_DESCRIPTOR_SIZE = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);

        let mut rtv_handle : D3D12_CPU_DESCRIPTOR_HANDLE = GSWAPCHAIN_HEAP.as_ref().unwrap().GetCPUDescriptorHandleForHeapStart();
        for idx in 0..GMAXFRAME
        {
            if let Ok(x) = GSWAPCHAIN.as_ref().unwrap().GetBuffer::<ID3D12Resource>(idx as u32)
            {
                GSWAPCHAIN_RESOURCE[idx] = Some(x);
                device.CreateRenderTargetView(GSWAPCHAIN_RESOURCE[idx].as_ref().unwrap(), None, rtv_handle);
                rtv_handle.ptr = rtv_handle.ptr + GRTV_DESCRIPTOR_SIZE as usize;
            }
        }

        GCURRENT_FRAME_INDEX = GSWAPCHAIN.as_ref().unwrap().GetCurrentBackBufferIndex();
    }
}

// function to create fence
fn create_fence()
{
    unsafe
    {
        if let Ok(x) = GD3D12_DEVICE.as_ref().unwrap().CreateFence::<ID3D12Fence>(0, D3D12_FENCE_FLAG_NONE)
        {
            // create fence event after CreateFence succeeded
            GMAIN_FENCE = Some(x);
            GMAIN_FENCE_VALUE = 1;
            if let Ok(x) = CreateEventW(None, FALSE, FALSE, None)
            {
                GMAIN_FENCE_EVENT = Some(x);
            }

            wait_for_gpu();
        }
    }
}

// function to initialize d3d12
pub fn initialize_d3d12(h_wnd : HWND, render_width : u32, render_height : u32) -> bool
{
    unsafe 
    {
        create_device();
        if GD3D12_DEVICE.is_none()
        {
            println!("D3D12 is not supported on this device!");
            return false;
        }

        create_command_buffers();
        if GMAIN_COMMAND_QUEUE.is_none() || GMAIN_COMMAND_ALLOCATOR.is_none() || GMAIN_COMMAND_LIST.is_none()
        {
            println!("Error during command buffers creation!");
            return false;
        }

        create_swapchain(h_wnd, render_width, render_height);
        if GSWAPCHAIN.is_none() || GSWAPCHAIN_HEAP.is_none()
        {
            println!("Error during swapchain creation!");
            return false;
        }

        create_fence();
        if GMAIN_FENCE.is_none() || GMAIN_FENCE_EVENT.is_none()
        {
            println!("Error during fence creation!");
            return false;
        }
    }

    return true;
}

// function to shutdown
pub fn shutdown()
{
    unsafe
    {
        wait_for_gpu();
        let _ = CloseHandle(GMAIN_FENCE_EVENT.as_ref());
    }
}

// wait for gpu fence
pub fn wait_for_gpu()
{
    unsafe
    {
        let main_fence  = GMAIN_FENCE.as_ref().unwrap();
        let prev_fence_value : u64 = GMAIN_FENCE_VALUE;
        let _ = GMAIN_COMMAND_QUEUE.as_ref().unwrap().Signal(main_fence, prev_fence_value);
        GMAIN_FENCE_VALUE = GMAIN_FENCE_VALUE + 1;

        if main_fence.GetCompletedValue() < prev_fence_value
        {
            let fence_event = GMAIN_FENCE_EVENT.as_ref();
            let _ = main_fence.SetEventOnCompletion(prev_fence_value, fence_event);
            WaitForSingleObject(fence_event, INFINITE);
        }

        // advance frame index
        GCURRENT_FRAME_INDEX = GSWAPCHAIN.as_ref().unwrap().GetCurrentBackBufferIndex();
    }
}

// update function
pub fn update()
{
    unsafe 
    {
        // print out all error messages stored in ID3D12QueueInfo
        if GDEBUG_INFO_QUEUE.is_some()
        {
            let debug_info_queue = GDEBUG_INFO_QUEUE.as_ref().unwrap();
            let error_message_count = debug_info_queue.GetNumStoredMessages();

            // print error message if there is any
            if error_message_count > 0
            {
                for idx in 0..error_message_count
                {
                    // first GetMessage() call to get the error byte length
                    let mut message_byte_length = 0;
                    let _ = debug_info_queue.GetMessage(idx, None, &mut message_byte_length);                    

                    // second GetMessage() to get the error and print it, be sure to allocate D3D12_MESSAGE with enough size!
                    let error_message : Option<*mut D3D12_MESSAGE> = Some(libc::malloc(message_byte_length) as *mut D3D12_MESSAGE);

                    if let Ok(()) = debug_info_queue.GetMessage(idx, error_message, &mut message_byte_length)
                    {
                        let unwrapped_message : *mut D3D12_MESSAGE = error_message.unwrap();
                        println!("{}", PCSTR::from_raw((*unwrapped_message).pDescription).display());
                    }

                    libc::free(error_message.unwrap() as *mut _ as *mut c_void);
                }

                // clear all printed messages
                debug_info_queue.ClearStoredMessages();
            }
        }
    }
}

// present the backbuffer
pub fn present()
{
    unsafe
    {
        let mut present_flags : DXGI_PRESENT = DXGI_PRESENT::default();
        if GSUPPORT_SCREEN_TEARING
        {
            present_flags = present_flags | DXGI_PRESENT_ALLOW_TEARING;
        }

        let _ = GSWAPCHAIN.as_ref().unwrap().Present(0, present_flags);
    }
}

// getter functions
pub fn get_device() -> &'static ID3D12Device
{
    unsafe 
    {
        return GD3D12_DEVICE.as_ref().unwrap();
    }
}

pub fn get_command_allocator() -> &'static ID3D12CommandAllocator
{
    unsafe
    {
        return GMAIN_COMMAND_ALLOCATOR.as_ref().unwrap();
    }
}

pub fn get_command_list() -> &'static ID3D12GraphicsCommandList
{
    unsafe
    {
        return GMAIN_COMMAND_LIST.as_ref().unwrap();
    }
}

pub fn get_command_queue() -> &'static ID3D12CommandQueue
{
    unsafe 
    {
        return GMAIN_COMMAND_QUEUE.as_ref().unwrap();
    }
}

pub fn get_back_buffer_rtv() -> D3D12_CPU_DESCRIPTOR_HANDLE
{
    unsafe
    {
        // offset the handle based on frame index
        let mut rtv_handle : D3D12_CPU_DESCRIPTOR_HANDLE = GSWAPCHAIN_HEAP.as_ref().unwrap().GetCPUDescriptorHandleForHeapStart();
        rtv_handle.ptr = rtv_handle.ptr + (GRTV_DESCRIPTOR_SIZE * GCURRENT_FRAME_INDEX) as usize;

        return rtv_handle;
    }
}

pub fn get_back_buffer_format() -> DXGI_FORMAT
{
    return GBACK_BUFFER_FORMAT;
}

pub fn get_back_buffer_resource() -> &'static Option<ID3D12Resource>
{
    unsafe
    {
        return &GSWAPCHAIN_RESOURCE[GCURRENT_FRAME_INDEX as usize];
    }
}