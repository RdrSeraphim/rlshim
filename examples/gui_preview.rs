pub mod auth {
    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    pub struct GameAccount {
        pub account_id: String,
        pub display_name: String,
        pub user_hash: String,
    }
}

#[path = "../src/logger.rs"]
mod logger;

#[allow(dead_code, unused_imports)]
#[path = "../src/gui.rs"]
mod gui;

use auth::GameAccount;
use eframe::egui::{self, Pos2, Rect, Vec2};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::{Arc, Mutex};

const URL_STEP1_INSTRUCTIONS: &str = "welcome to rlshim!\n\nto log in, open the link below in your browser, log in with your Jagex account, and wait to be redirected to a localhost URL (it will fail to load, that's normal).";
const URL_STEP1_PT2: &str =
    "copy the full localhost URL from your browser's address bar and paste it here:";
const URL_STEP1_SAMPLE: &str = "https://account.jagex.com/oauth2/auth?client_id=com_jagex_auth_desktop_launcher&code_challenge=abcdef1234567890abcdef1234567890abcdef12345&code_challenge_method=S256&prompt=login&redirect_uri=https%3A%2F%2Fsecure.runescape.com%2Fm%3Dweblogin%2Flauncher-redirect&response_type=code&scope=openid+offline+gamesso.token.create+user.profile.read&state=xyz987654321";
const URL_STEP1_PLACEHOLDER: &str = "http://localhost/?code=...";

const URL_STEP2_INSTRUCTIONS: &str = "round two, the last one. this is just so Jagex knows you're not a robot and are eligible to receive a session ID.\n\nthe process is a bit similar, open the link below, get through cloudflare (login shouldn't be necessary), get redirected to a localhost URL";
const URL_STEP2_PT2: &str = "like before, copy the full localhost URL and paste it here:";
const URL_STEP2_SAMPLE: &str = "https://account.jagex.com/oauth2/auth?id_token_hint=eyJhbGciOiJSUzI1NiIsImtpZCI6IjEifQ.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signature&nonce=abcdef1234567890abcdef1234567890&prompt=consent&redirect_uri=http%3A%2F%2Flocalhost&response_type=id_token%20code&state=1234567890abcdef1234567890abcdef&client_id=1fddee4e-b100-4f4e-b2b0-097f9088f9d2&scope=openid%20offline";
const URL_STEP2_PLACEHOLDER: &str = "http://localhost/#code=...";

const WARN_JAVA: &str = "java 11+ is not installed or not in PATH";
const WARN_HOME: &str = "failed to find or create ~/.runelite, this is likely a permissions issue.\nensure that the user running this application has write access to their home directory.";
const WARN_KEYRING: &str = "rlshim could not find a valid keyring. credentials cannot\nbe stored securely at this point. please install libsecret and a keyring\nmanager (e.g. gnome-keyring, kwallet, etc.) to proceed further.";
const WARN_RUNELITE: &str = "failed to ensure runelite.jar exists\n\nensure that you have write permissions to ~/.runelite and that there is sufficient disk space.";

struct SoftTex {
    w: usize,
    h: usize,
    pixels: Vec<[u8; 4]>,
}

impl SoftTex {
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        if self.w == 0 || self.h == 0 {
            return [1.0, 1.0, 1.0, 1.0];
        }
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let fx = u * (self.w as f32) - 0.5;
        let fy = v * (self.h as f32) - 0.5;
        let x0 = fx.floor() as isize;
        let y0 = fy.floor() as isize;
        let tx = fx - (x0 as f32);
        let ty = fy - (y0 as f32);

        let fetch = |x: isize, y: isize| -> [f32; 4] {
            let cx = x.clamp(0, (self.w as isize) - 1) as usize;
            let cy = y.clamp(0, (self.h as isize) - 1) as usize;
            let p = self.pixels[cy * self.w + cx];
            [
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
                p[3] as f32 / 255.0,
            ]
        };

        let p00 = fetch(x0, y0);
        let p10 = fetch(x0 + 1, y0);
        let p01 = fetch(x0, y0 + 1);
        let p11 = fetch(x0 + 1, y0 + 1);
        let mut out = [0.0; 4];
        for c in 0..4 {
            let v0 = p00[c] * (1.0 - tx) + p10[c] * tx;
            let v1 = p01[c] * (1.0 - tx) + p11[c] * tx;
            out[c] = v0 * (1.0 - ty) + v1 * ty;
        }
        out
    }
}

fn render_to_png<F: FnMut(&egui::Context)>(width: u32, height: u32, out_path: &Path, mut ui_fn: F) {
    let ctx = egui::Context::default();
    gui::configure_fonts_and_style(&ctx);
    let mut textures: HashMap<egui::TextureId, SoftTex> = HashMap::new();
    let mut last_primitives = Vec::new();

    for _ in 0..2 {
        let raw_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(width as f32, height as f32),
            )),
            ..Default::default()
        };
        let full_output = ctx.run(raw_input, |ctx| ui_fn(ctx));
        for (id, delta) in full_output.textures_delta.set {
            let [w, h] = delta.image.size();
            let pixels: Vec<[u8; 4]> = match &delta.image {
                egui::ImageData::Color(c) => c.pixels.iter().map(|p| p.to_array()).collect(),
                egui::ImageData::Font(f) => f.srgba_pixels(None).map(|p| p.to_array()).collect(),
            };
            if let Some(pos) = delta.pos {
                if let Some(existing) = textures.get_mut(&id) {
                    for y in 0..h {
                        for x in 0..w {
                            let dst_x = pos[0] + x;
                            let dst_y = pos[1] + y;
                            if dst_x < existing.w && dst_y < existing.h {
                                existing.pixels[dst_y * existing.w + dst_x] = pixels[y * w + x];
                            }
                        }
                    }
                }
            } else {
                textures.insert(id, SoftTex { w, h, pixels });
            }
        }
        last_primitives = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
    }

    let mut fb = vec![[26u8, 26u8, 26u8]; (width * height) as usize];
    let edge = |ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32| -> f32 {
        (px - ax) * (by - ay) - (py - ay) * (bx - ax)
    };

    for clipped in last_primitives {
        let clip = clipped.clip_rect;
        let clip_x0 = (clip.min.x.floor() as i32).max(0);
        let clip_y0 = (clip.min.y.floor() as i32).max(0);
        let clip_x1 = (clip.max.x.ceil() as i32).min(width as i32);
        let clip_y1 = (clip.max.y.ceil() as i32).min(height as i32);

        let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else {
            continue;
        };
        let tex = textures.get(&mesh.texture_id);

        for tri in mesh.indices.chunks_exact(3) {
            let v0 = &mesh.vertices[tri[0] as usize];
            let v1 = &mesh.vertices[tri[1] as usize];
            let v2 = &mesh.vertices[tri[2] as usize];

            let min_x = v0.pos.x.min(v1.pos.x).min(v2.pos.x);
            let max_x = v0.pos.x.max(v1.pos.x).max(v2.pos.x);
            let min_y = v0.pos.y.min(v1.pos.y).min(v2.pos.y);
            let max_y = v0.pos.y.max(v1.pos.y).max(v2.pos.y);

            let x0 = (min_x.floor() as i32).max(clip_x0);
            let y0 = (min_y.floor() as i32).max(clip_y0);
            let x1 = (max_x.ceil() as i32).min(clip_x1);
            let y1 = (max_y.ceil() as i32).min(clip_y1);

            let area = edge(v0.pos.x, v0.pos.y, v1.pos.x, v1.pos.y, v2.pos.x, v2.pos.y);
            if area.abs() < 1e-6 {
                continue;
            }

            let c0 = v0.color.to_array().map(|v| v as f32 / 255.0);
            let c1 = v1.color.to_array().map(|v| v as f32 / 255.0);
            let c2 = v2.color.to_array().map(|v| v as f32 / 255.0);

            for y in y0..y1 {
                for x in x0..x1 {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    let w0 = edge(v1.pos.x, v1.pos.y, v2.pos.x, v2.pos.y, px, py) / area;
                    let w1 = edge(v2.pos.x, v2.pos.y, v0.pos.x, v0.pos.y, px, py) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                        let u = w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x;
                        let v = w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y;
                        let tcol = tex.map_or([1.0; 4], |t| t.sample(u, v));

                        let r = (w0 * c0[0] + w1 * c1[0] + w2 * c2[0]) * tcol[0];
                        let g = (w0 * c0[1] + w1 * c1[1] + w2 * c2[1]) * tcol[1];
                        let b = (w0 * c0[2] + w1 * c1[2] + w2 * c2[2]) * tcol[2];
                        let a = (w0 * c0[3] + w1 * c1[3] + w2 * c2[3]) * tcol[3];

                        let dst = &mut fb[(y as u32 * width + x as u32) as usize];
                        dst[0] = ((r + (dst[0] as f32 / 255.0) * (1.0 - a)) * 255.0 + 0.5)
                            .clamp(0.0, 255.0) as u8;
                        dst[1] = ((g + (dst[1] as f32 / 255.0) * (1.0 - a)) * 255.0 + 0.5)
                            .clamp(0.0, 255.0) as u8;
                        dst[2] = ((b + (dst[2] as f32 / 255.0) * (1.0 - a)) * 255.0 + 0.5)
                            .clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }

    let mut img = image::RgbImage::new(width, height);
    for (i, p) in fb.into_iter().enumerate() {
        img.put_pixel((i as u32) % width, (i as u32) / width, image::Rgb(p));
    }
    img.save(out_path).expect("failed to write PNG");
    println!("Wrote {}", out_path.display());
}

fn dump_all_pngs(out_dir: &Path) {
    std::fs::create_dir_all(out_dir).expect("failed to create output directory");

    let mut char_app = None;
    render_to_png(600, 600, &out_dir.join("char_select.png"), |ctx| {
        let app = char_app.get_or_insert_with(|| gui::CharSelectApp {
            bg: gui::load_background_texture(ctx),
            accounts: sample_accounts(),
            result_slot: Arc::new(Mutex::new(None)),
        });
        app.draw(ctx);
    });

    let mut url1_app = None;
    render_to_png(600, 600, &out_dir.join("url_prompt_step1.png"), |ctx| {
        let app = url1_app.get_or_insert_with(|| gui::UrlPromptApp {
            bg: gui::load_background_texture(ctx),
            instructions: URL_STEP1_INSTRUCTIONS.into(),
            instructions_pt_2: URL_STEP1_PT2.into(),
            url_to_open: URL_STEP1_SAMPLE.into(),
            url_placeholder: URL_STEP1_PLACEHOLDER.into(),
            input_buf: String::new(),
            opened_at: None,
            copied_at: None,
            result_slot: Arc::new(Mutex::new(None)),
        });
        app.draw(ctx);
    });

    let mut url2_app = None;
    render_to_png(600, 600, &out_dir.join("url_prompt_step2.png"), |ctx| {
        let app = url2_app.get_or_insert_with(|| gui::UrlPromptApp {
            bg: gui::load_background_texture(ctx),
            instructions: URL_STEP2_INSTRUCTIONS.into(),
            instructions_pt_2: URL_STEP2_PT2.into(),
            url_to_open: URL_STEP2_SAMPLE.into(),
            url_placeholder: URL_STEP2_PLACEHOLDER.into(),
            input_buf: String::new(),
            opened_at: None,
            copied_at: None,
            result_slot: Arc::new(Mutex::new(None)),
        });
        app.draw(ctx);
    });

    for (name, msg, height) in [
        ("warn_java.png", WARN_JAVA, 230),
        ("warn_home.png", WARN_HOME, 260),
        ("warn_keyring.png", WARN_KEYRING, 260),
        ("warn_runelite.png", WARN_RUNELITE, 260),
    ] {
        let mut warn_app = None;
        render_to_png(600, height, &out_dir.join(name), |ctx| {
            let app = warn_app.get_or_insert_with(|| gui::WarningApp {
                bg: gui::load_background_texture(ctx),
                warning_msg: format!("warning: {msg}"),
            });
            app.draw(ctx);
        });
    }
}

fn sample_accounts() -> Vec<GameAccount> {
    vec![
        GameAccount {
            account_id: "1".into(),
            display_name: "Seraphim IRL".into(),
            user_hash: "h1".into(),
        },
        GameAccount {
            account_id: "2".into(),
            display_name: "IM RdrPhim".into(),
            user_hash: "h2".into(),
        },
    ]
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--dump") {
        let dir = args.get(1).map_or("/tmp/rlshim-gui-preview", String::as_str);
        dump_all_pngs(Path::new(dir));
        return;
    }

    let mode = args.first().map_or("all", String::as_str);
    match mode {
        "char" => {
            let _ = gui::prompt_for_character(&sample_accounts());
        }
        "url" | "url1" => {
            let _ = gui::prompt_for_url(
                "rlshim - initial login",
                URL_STEP1_INSTRUCTIONS,
                URL_STEP1_PT2,
                URL_STEP1_SAMPLE,
                URL_STEP1_PLACEHOLDER,
            );
        }
        "url2" => {
            let _ = gui::prompt_for_url(
                "rlshim - initial login 2 electric boogaloo",
                URL_STEP2_INSTRUCTIONS,
                URL_STEP2_PT2,
                URL_STEP2_SAMPLE,
                URL_STEP2_PLACEHOLDER,
            );
        }
        "warn-java" => gui::warning_prompt("java error", WARN_JAVA, 230),
        "warn-home" => gui::warning_prompt("home error", WARN_HOME, 260),
        "warn-keyring" => gui::warning_prompt("keyring error", WARN_KEYRING, 260),
        "warn-runelite" => gui::warning_prompt("runelite error", WARN_RUNELITE, 260),
        "all" => {
            let _ = gui::prompt_for_character(&sample_accounts());
            let _ = gui::prompt_for_url(
                "rlshim - initial login",
                URL_STEP1_INSTRUCTIONS,
                URL_STEP1_PT2,
                URL_STEP1_SAMPLE,
                URL_STEP1_PLACEHOLDER,
            );
            let _ = gui::prompt_for_url(
                "rlshim - initial login 2 electric boogaloo",
                URL_STEP2_INSTRUCTIONS,
                URL_STEP2_PT2,
                URL_STEP2_SAMPLE,
                URL_STEP2_PLACEHOLDER,
            );
            gui::warning_prompt("java error", WARN_JAVA, 230);
            gui::warning_prompt("home error", WARN_HOME, 260);
            gui::warning_prompt("keyring error", WARN_KEYRING, 260);
            gui::warning_prompt("runelite error", WARN_RUNELITE, 260);
        }
        _ => {
            eprintln!(
                "Usage: cargo run --example gui_preview -- [all|char|url|url2|warn-java|warn-home|warn-keyring|warn-runelite|--dump <dir>]"
            );
            std::process::exit(1);
        }
    }
}
