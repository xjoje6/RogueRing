//! Guarda una captura PNG de la ventana del juego con Windows Graphics Capture.
//! Funciona con la ventana tapada o en otro monitor (no minimizada) y no le quita el foco a nadie.
//!
//! Uso: captura <salida.png> [proceso]   (proceso por defecto: eldenring.exe)

use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::encoder::ImageFormat;
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

struct Shot {
    out: String,
}

impl GraphicsCaptureApiHandler for Shot {
    type Flags = String;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { out: ctx.flags })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, control: InternalCaptureControl) -> Result<(), Self::Error> {
        frame.save_as_image(&self.out, ImageFormat::Png)?;
        control.stop();
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "captura.png".into());
    let proc = args.get(2).cloned().unwrap_or_else(|| "eldenring.exe".into()).to_lowercase();
    // Si la ventana está minimizada no llegan frames: salir con error en vez de quedarse colgado.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(8));
        eprintln!("error: sin imagen en 8 s (¿ventana minimizada?)");
        std::process::exit(2);
    });
    let win = Window::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter(|w| w.process_name().map(|p| p.to_lowercase() == proc).unwrap_or(false))
        .max_by_key(|w| w.width().unwrap_or(0) * w.height().unwrap_or(0));
    let Some(win) = win else {
        eprintln!("error: no hay ventana de {proc}");
        std::process::exit(1);
    };
    let settings = Settings::new(
        win,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        out.clone(),
    );
    match Shot::start(settings) {
        Ok(()) => println!("{out}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
