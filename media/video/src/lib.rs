//! OriginLM's 35-second silent film. Captured UI is explicitly synthetic.
#![recursion_limit = "512"]
#[path = "brand.generated.rs"]
mod brand;
use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video, SyncVideoFrameInput, FFramesSyncedVideoFrame};
pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
const BG: &str = brand::DARK_BACKGROUND;
const FG: &str = brand::PAPER;
const AMBER: &str = brand::AMBER;
const MUTED: &str = brand::DARK_MUTED;
const FONT: &str = brand::FONT;
fn ease(time: f32) -> f32 { 1. - (1. - time.clamp(0., 1.)).powi(4) }
fn spring(time: f32) -> f32 {
    if time <= 0. { return 0.; }
    let omega = 39_f32.sqrt();
    1. - (-9. * time).exp() * ((omega * time).cos() + 9. / omega * (omega * time).sin())
}
fn mix(a: f32, b: f32, t: f32) -> f32 { a + (b-a)*t }

pub struct OriginlmFilmVideo { shots: Vec<Shot> }
impl OriginlmFilmVideo {
    pub fn new(_: &str) -> Self {
        let shots = [
            ("Problem", 3., "Your AI is everywhere.", "Whatever AI you already have."),
            ("See", 7., "Know all your AI.", "Usage. Reset times. Models. Estimated costs, clearly labeled."),
            ("Build", 9., "Build with it.", "One dependency. A secure local runtime. Your users’ own AI."),
            ("Connect", 4., "Permission belongs to you.", "An example grant. No account is connected or used in this film."),
            ("Install", 5., "One dependency to begin.", "Local package preview. Registry release is not published yet."),
            ("Origin", 7., "See your AI.", "Build with it."),
        ].into_iter().enumerate().map(|(kind,(name,seconds,title,caption))| Shot {kind,name,seconds,title,caption}).collect();
        Self { shots }
    }
}
impl std::fmt::Debug for OriginlmFilmVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("OriginlmFilmVideo").finish() }
}
impl Video for OriginlmFilmVideo {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;
    fn duration(&self) -> Duration<'_> { Duration::Auto }
    fn audio(&self) -> AudioMap<'_> { AudioMap::none() }
    fn define_scenes(&self) -> Scenes<'_> { Scenes::from(self.shots.iter().map(|s|s as &dyn Scene).collect::<Vec<_>>()) }
    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        fframes::svgr!(<svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT}>
            <rect width={WIDTH} height={HEIGHT} fill={BG}/>
            {ctx.render_scenes(&frame)}
        </svg>)
    }
}
#[derive(Debug)]
struct Shot { kind: usize, name: &'static str, seconds: f32, title: &'static str, caption: &'static str }
impl Scene for Shot {
    fn name(&self) -> &'static str {self.name}
    fn duration(&self) -> Duration<'_> {Duration::Seconds(self.seconds)}
    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let content = match self.kind {
            0 => {
                let names = ["Claude Code", "Codex", "Gemini", "OpenCode", "Ollama", "LM Studio"];
                let chaos = [(90.,330.,-12.),(950.,290.,8.),(1390.,420.,-7.),(180.,760.,9.),(760.,680.,-10.),(1360.,800.,7.)];
                names.iter().enumerate().map(|(index,name)| {
                    let progress = spring(t - 0.65 - index as f32 * 0.065);
                    let x = mix(chaos[index].0, 240. + (index % 3) as f32 * 500., progress);
                    let y = mix(chaos[index].1, 430. + (index / 3) as f32 * 180., progress);
                    let angle = mix(chaos[index].2, 0., progress);
                    let transform = format!("translate({x} {y}) rotate({angle} 220 60)");
                    fframes::svgr!(<g transform={transform}>
                        <rect width="440" height="120" rx="18" fill={BG} stroke={AMBER} stroke-width="2"/>
                        <text x="220" y="76" text-anchor="middle" fill={FG} font-family={FONT} font-size="42" font-weight="550">{*name}</text>
                    </g>)
                }).collect::<Svgr>()
            },
            1 => {
                let enter = ease(t / 0.3);
                let macro_ring = ease((t - 1.4) / 1.4);
                let macro_cost = ease((t - 4.) / 1.6);
                let scale = 1. + macro_ring * 0.35;
                let x = 170. - macro_ring * 80. - macro_cost * 450.;
                let y = 280. + (1. - enter) * 50.;
                let transform = format!("translate({x} {y}) scale({scale})");
                let image = ctx.get_image("dashboard-detail.png").expect("captured example overview");
                fframes::svgr!(<g>
                    <defs><clipPath id="dashboard-clip"><rect x="120" y="230" width="1680" height="700" rx="12"/></clipPath></defs>
                    <g clip-path="url(#dashboard-clip)"><g transform={transform} opacity={enter}>
                        <image href={image.href()} width="1580" height="560"/>
                    </g></g>
                </g>)
            },
            2 => {
                let input = SyncVideoFrameInput { start_from: 0., looping: false, editor_fallback_image: None };
                let image = frame.get_synced_video_frame(ctx, "studio.mp4", &input).expect("render the shared studio shot before building the film");
                let image = image.into_image();
                fframes::svgr!(<image href={image.href()} width={WIDTH} height={HEIGHT}/>)
            },
            3 => {
                let grant = ctx.get_image("runtime.png").expect("captured public runtime panel");
                let enter = spring(t);
                let flipped = ease((t - 0.85) / 0.22);
                let offset = (1. - enter) * 80.;
                fframes::svgr!(<g>
                    <g transform={format!("translate(140 {})", 350. + offset)}>
                        <rect width="760" height="440" rx="20" fill={BG} stroke={AMBER} stroke-width="2"/>
                        <text x="50" y="90" fill={FG} font-family={FONT} font-size="48" font-weight="600">"Your app"</text>
                        <text x="50" y="170" fill={MUTED} font-family={FONT} font-size="32">"Example authorization"</text>
                        <text x="50" y="265" fill={FG} font-family={FONT} font-size="38">"Allow discovery"</text>
                        <rect x="570" y="220" width="130" height="58" rx="29" fill={AMBER} opacity={0.35 + flipped * 0.65}/>
                        <circle cx={601. + flipped * 68.} cy="249" r="22" fill={FG}/>
                        <text x="50" y="365" fill={MUTED} font-family={FONT} font-size="30">"Execution requires its own grant."</text>
                    </g>
                    <g transform={format!("translate(990 {})", 350. - offset)}>
                        <image href={grant.href()} width="790" height="440"/>
                    </g>
                    <path d="M910 570H975" stroke={AMBER} stroke-width="2" opacity={flipped}/>
                </g>)
            },
            4 => {
                let (first, second, third) = (spring(t), spring(t - 0.14), spring(t - 0.28));
                fframes::svgr!(<g>
                    <text x={150. + (1. - first) * 80.} y="430" fill={AMBER} font-family={FONT} font-size="40" opacity={first.clamp(0.,1.)}>"npm install ./originlm-*.tgz"</text>
                    <path d="M150 485H1770" stroke={AMBER} stroke-width="2"/>
                    <text x={150. + (1. - second) * 80.} y="590" fill={FG} font-family={FONT} font-size="40" opacity={second.clamp(0.,1.)}>"import { createLocalAI } from 'originlm';"</text>
                    <text x={150. + (1. - third) * 80.} y="700" fill={FG} font-family={FONT} font-size="46" opacity={third.clamp(0.,1.)}>"const ai = await createLocalAI();"</text>
                    <text x={150. + (1. - third) * 80.} y="810" fill={AMBER} font-family={FONT} font-size="46" opacity={third.clamp(0.,1.)}>"const snapshot = await ai.snapshot.get();"</text>
                </g>)
            },
            _ => {
                let logo = ctx.get_image("originlm-white.png").expect("OriginLM logo");
                let enter = ease(t / 0.5);
                let y = (1. - enter) * 35.;
                fframes::svgr!(<g transform={format!("translate(0 {y})")}>
                    <image href={logo.href()} x="150" y="155" width="540" height="155"/>
                    <text x="150" y="535" fill={FG} font-family={FONT} font-size="150" font-weight="650">"See your AI."</text>
                    <text x="150" y="710" fill={AMBER} font-family={FONT} font-size="150" font-weight="650">"Build with it."</text>
                    <text x="150" y="900" fill={FG} font-family={FONT} font-size="42" opacity={ease((t - 2.3) / 0.35)}>"github.com/thomaslittle/originlm"</text>
                    <text x="150" y="975" fill={MUTED} font-family={FONT} font-size="30">"Source preview. No OriginLM account."</text>
                </g>)
            },
        };
        let heading = if self.kind == 5 { Svgr::default() } else { fframes::svgr!(<g>
            <text x="140" y="150" fill={FG} font-family={FONT} font-size="88" font-weight="650">{self.title}</text>
            <text x="1770" y="70" text-anchor="end" fill={MUTED} font-family={FONT} font-size="26">{if self.kind == 1 || self.kind == 2 {"Example data"} else {"OriginLM"}}</text>
            <rect x="0" y="1000" width={WIDTH} height="80" fill={BG}/>
            <text x="140" y="1050" fill={MUTED} font-family={FONT} font-size="29">{self.caption}</text>
        </g>) };
        fframes::svgr!(<g>{content}{heading}</g>)
    }
}
