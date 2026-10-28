from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
logo_svg = (root / "src/assets/brand/originlm-horizontal-dark.svg").read_text()
inner = re.search(r"<g fill=\"#FFFFFF\">.*</g>", logo_svg, re.S).group(0)

def layer(x, y, w, h, label, sub, dash=False):
    return f'''
  <g>
    <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" fill="url(#glass)" stroke="#8a7160" stroke-opacity="0.55"/>
    <rect x="{x+14}" y="{y+h-12}" width="{w-28}" height="2" rx="1" fill="#eba31a" opacity="{0.5 if dash else 0.26}"/>
    <text x="{x+w+26}" y="{y+h/2-2}" fill="#b0a8a1" font-family="ui-monospace, monospace" font-size="13">{label}</text>
    <text x="{x+w+26}" y="{y+h/2+17}" fill="#6f6460" font-family="ui-monospace, monospace" font-size="10">{sub}</text>
  </g>'''

packets = "\n".join(
    f'<circle cx="{560 + ((i * 67) % 120) - 60}" cy="{140 + i * 62}" r="{3.2 + (i % 3)}" fill="{"#f7bd4b" if i % 2 == 0 else "#a9b29f"}" opacity="0.9"/>'
    for i in range(9)
)

nodes = "\n".join(
    f'<g><circle cx="{300 + i * 92}" cy="700" r="{10 + (i % 2) * 2}" fill="none" stroke="{"#eba31a" if i % 3 else "#a9b29f"}" stroke-opacity="0.35"/><circle cx="{300 + i * 92}" cy="700" r="{4.5 + (i % 2)}" fill="{"#f7bd4b" if i % 2 == 0 else "#a9b29f"}"/></g>'
    for i in range(7)
)

hero = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 760" width="1200" height="760" role="img" aria-labelledby="t">
<title id="t">The OriginLM stack, exploded</title>
<defs>
  <radialGradient id="glow" cx="62%" cy="34%" r="60%">
    <stop offset="0" stop-color="#eba31a" stop-opacity="0.13"/>
    <stop offset="1" stop-color="#eba31a" stop-opacity="0"/>
  </radialGradient>
  <linearGradient id="glass" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#faf9f5" stop-opacity="0.10"/>
    <stop offset="1" stop-color="#faf9f5" stop-opacity="0.035"/>
  </linearGradient>
  <pattern id="grid" width="48" height="48" patternUnits="userSpaceOnUse">
    <path d="M48 0H0V48" fill="none" stroke="#faf9f5" stroke-opacity="0.05"/>
  </pattern>
</defs>
<rect width="1200" height="760" fill="#191516"/>
<rect width="1200" height="760" fill="url(#grid)"/>
<rect width="1200" height="760" fill="url(#glow)"/>
<g transform="rotate(-0.8 600 380)">
  <g>
    <rect x="150" y="66" width="520" height="122" rx="16" fill="url(#glass)" stroke="#f7bd4b" stroke-opacity="0.7"/>
    <circle cx="176" cy="88" r="4" fill="#eba31a"/><circle cx="190" cy="88" r="4" fill="#6f625c"/><circle cx="204" cy="88" r="4" fill="#6f625c"/>
    <text x="176" y="136" fill="#faf9f5" font-family="Red Hat Display, system-ui, sans-serif" font-size="22" font-weight="600">Your app</text>
    <text x="176" y="160" fill="#8a817b" font-family="ui-monospace, monospace" font-size="11">tauri · electron · gpui · web</text>
  </g>
  <g>
    <rect x="712" y="62" width="330" height="142" rx="16" fill="url(#glass)" stroke="#8a7160" stroke-opacity="0.6"/>
    <text x="734" y="96" fill="#faf9f5" font-family="Red Hat Display, system-ui, sans-serif" font-size="16" font-weight="600">Usage dashboard</text>
    <rect x="734" y="112" width="120" height="8" rx="4" fill="#faf9f5" opacity="0.10"/>
    <rect x="734" y="112" width="78" height="8" rx="4" fill="#eba31a" opacity="0.8"/>
    <rect x="734" y="132" width="120" height="8" rx="4" fill="#faf9f5" opacity="0.10"/>
    <rect x="734" y="132" width="42" height="8" rx="4" fill="#a9b29f" opacity="0.8"/>
    <text x="734" y="176" fill="#8a817b" font-family="ui-monospace, monospace" font-size="10">quota · reset · pace</text>
    <text x="898" y="96" fill="#eba31a" font-family="ui-monospace, monospace" font-size="10">SAME RUNTIME</text>
  </g>
{layer(226, 232, 560, 84, "SDK", "rust · npm · python")}
{layer(210, 340, 592, 72, "Transport", "stdio · ipc · native messaging")}
{layer(192, 436, 628, 88, "Runtime", "grants · discovery · sessions")}
{layer(176, 548, 660, 72, "Adapters", "one identity model")}
  <g>
    <rect x="150" y="646" width="712" height="96" rx="16" fill="url(#glass)" stroke="#8a7160" stroke-opacity="0.55"/>
    <text x="170" y="674" fill="#faf9f5" font-family="Red Hat Display, system-ui, sans-serif" font-size="15" font-weight="600">Providers</text>
    <text x="170" y="692" fill="#6f6460" font-family="ui-monospace, monospace" font-size="10">Claude · Codex · Cursor · Ollama …</text>
{nodes}
  </g>
  <g opacity="0.8">{packets}</g>
</g>
<text x="1082" y="600" fill="#8a817b" font-family="ui-monospace, monospace" font-size="11" text-anchor="end">one dependency</text>
<text x="1082" y="620" fill="#8a817b" font-family="ui-monospace, monospace" font-size="11" text-anchor="end">one local runtime</text>
</svg>
'''
(root / "public/hero-poster.svg").write_text(hero)

chapters = [
    ("01", "Know your AI", "Every client, account and quota in one local view."),
    ("02", "Build on it", "One dependency from providers to your app."),
    ("03", "Connect", "Discovery, a grant, and the connected badge."),
]

chapter_svg = "\n".join(
    f'''  <g>
    <text x="96" y="{560 + i * 68}" fill="#eba31a" font-family="ui-monospace, monospace" font-size="12">{num}</text>
    <text x="132" y="{560 + i * 68}" fill="#faf9f5" font-family="Red Hat Display, system-ui, sans-serif" font-size="19" font-weight="500">{title}</text>
    <text x="132" y="{584 + i * 68}" fill="#8a817b" font-family="Red Hat Display, system-ui, sans-serif" font-size="13">{body}</text>
  </g>'''
    for i, (num, title, body) in enumerate(chapters)
)

mini = "\n".join(
    f'<rect x="{980}" y="{210 + i * 52}" width="{330 - i * 14}" height="32" rx="8" fill="url(#glass)" stroke="#8a7160" stroke-opacity="0.5"/>'
    f'<circle cx="{1000}" cy="{226 + i * 52}" r="3.5" fill="{"#eba31a" if i < 5 else "#a9b29f"}"/>'
    for i in range(6)
)

video = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 900" width="1600" height="900" role="img" aria-labelledby="vt">
<title id="vt">OriginLM product film poster</title>
<defs>
  <radialGradient id="vglow" cx="76%" cy="45%" r="55%">
    <stop offset="0" stop-color="#eba31a" stop-opacity="0.18"/>
    <stop offset="1" stop-color="#eba31a" stop-opacity="0"/>
  </radialGradient>
  <linearGradient id="glass" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#faf9f5" stop-opacity="0.12"/>
    <stop offset="1" stop-color="#faf9f5" stop-opacity="0.04"/>
  </linearGradient>
  <pattern id="vgrid" width="56" height="56" patternUnits="userSpaceOnUse">
    <path d="M56 0H0V56" fill="none" stroke="#faf9f5" stroke-opacity="0.045"/>
  </pattern>
</defs>
<rect width="1600" height="900" fill="#171314"/>
<rect width="1600" height="900" fill="url(#vgrid)"/>
<rect width="1600" height="900" fill="url(#vglow)"/>
<g transform="translate(96 84) scale(0.155)">{inner}</g>
<text x="96" y="250" fill="#faf9f5" font-family="Red Hat Display, system-ui, sans-serif" font-size="70" font-weight="600" letter-spacing="-2">Know your AI.</text>
<text x="96" y="328" fill="#eba31a" font-family="Red Hat Display, system-ui, sans-serif" font-size="70" font-weight="600" letter-spacing="-2">Build on it.</text>
<text x="96" y="396" fill="#b0a8a1" font-family="Red Hat Display, system-ui, sans-serif" font-size="21">All your AI in one place. One dependency for every app.</text>
<g transform="rotate(-1 1200 450)">
{mini}
</g>
{chapter_svg}
<text x="96" y="824" fill="#6f6460" font-family="ui-monospace, monospace" font-size="12">originlm.dev · 52 seconds · silent</text>
<circle cx="1450" cy="800" r="30" fill="#eba31a"/>
<path d="M1442 786 L1464 800 L1442 814 Z" fill="#231f20"/>
</svg>
'''
(root / "public/video-poster.svg").write_text(video)
print("posters written", (root / "public/hero-poster.svg").stat().st_size, (root / "public/video-poster.svg").stat().st_size)
