// My Rust + D3D12 Initialization mini project.
// Feel free to use it if you want start graphics programming with Rust!

// main.rs - The entry point of the app, mainly for window initialization and setup game loop.

use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;
use windows_sys::*;
use std::mem;

mod graphic_device;
mod hello_world_triangle;

// define window proc function for the Win32 messages
unsafe extern "system" fn wnd_proc(h_wnd : HWND, message : u32, w_param : WPARAM, l_param : LPARAM) -> LRESULT
{
    match message
    {
        WM_DESTROY => 
        {
            PostQuitMessage(0);
            LRESULT::default()
        }
        _ => return DefWindowProcW(h_wnd,message,w_param,l_param),
    }
}

// entry point of the app
fn main()
{
    unsafe 
    {
        // get app instance, register class and create window
        // we don't have int WINAPI wWinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, PWSTR pCmdLine, int nCmdShow); in Rust
        // so it's necessary to get the HINSTANCE from module handle directly.
        let app_instance : HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let app_class_name = PCWSTR::from_raw(w!("Rust D3D12"));

        // setup WNDCLASSEXW struct, mem::size_of is the Rust version "sizeof"
        let app_class = WNDCLASSEXW
        {
            cbSize : mem::size_of::<WNDCLASSEXW>().try_into().unwrap(),
            style : CS_HREDRAW | CS_VREDRAW,
            cbClsExtra : 0,
            cbWndExtra : 0,
            hInstance : app_instance,
            hIcon : HICON::default(),
            hCursor : LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground : GetSysColorBrush(COLOR_WINDOWFRAME),
            lpszMenuName : PCWSTR::null(),
            lpszClassName : app_class_name,
            hIconSm : HICON::default(),
            lpfnWndProc : Some(wnd_proc),
        };

        RegisterClassExW(&app_class);

        // fixed at 1080p and disabled window resizing and maximizing for now
        let render_width : u32 = 1920;
        let render_height : u32 = 1080;
        let app_window = CreateWindowExW(WINDOW_EX_STYLE::default(), app_class_name, PCWSTR::from_raw(w!("Rust D3D12"))
        , WS_OVERLAPPED | WS_MINIMIZEBOX | WS_SYSMENU, 0, 0, render_width as i32, render_height as i32, None, None, app_instance, None).unwrap();

        // initialize graphic device
        if !graphic_device::initialize_d3d12(app_window, render_width, render_height)
        {
            return;
        }

        // initialize demo resources
        hello_world_triangle::create_pipeline();

        // show the window and enter the game loop after window and graphic device are created.
        let _ = ShowWindow(app_window, SW_SHOW);
        let mut msg = MSG::default();
        
        while msg.message != WM_QUIT
        {
            if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool()
            {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            else
            {    
                graphic_device::update();
                hello_world_triangle::render(render_width, render_height);

                // present and wait GPU fence. just for demo, it's not the best way to do this.
                // doing a ring-buffer workflow for frame resources is the way for better CPU-GPU efficiency.
                graphic_device::present();
                graphic_device::wait_for_gpu();
            }
        }

        graphic_device::shutdown();
    }
}
