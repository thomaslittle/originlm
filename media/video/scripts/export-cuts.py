#!/usr/bin/env python3
"""Encode silent film derivatives locally. Requires ffmpeg with VP9 and H.264."""
from pathlib import Path
import os
import subprocess

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'media/video/out'
FILM = OUT / 'originlm-1080p.mp4'
FONT = ROOT / 'media/video/media/RedHatDisplay-Variable.ttf'
FFMPEG = os.environ.get('FFMPEG', 'ffmpeg')
ENCODER = os.environ.get('VIDEO_ENCODER', 'libx264')
H264 = ['-c:v', ENCODER, *(['-b:v', '10M'] if ENCODER == 'libopenh264' else ['-crf', '19']), '-pix_fmt', 'yuv420p', '-movflags', '+faststart']


def run(args):
    subprocess.run([FFMPEG, '-hide_banner', '-loglevel', 'error', '-y', *map(str, args)], check=True)


def webm(source, target):
    run(['-i', source, '-c:v', 'libvpx-vp9', '-crf', '31', '-b:v', '0', '-row-mt', '1', '-threads', '2', '-an', target])


def text(label, y, size, color='faf9f5'):
    return f"drawtext=fontfile='{FONT}':text='{label}':fontcolor=0x{color}:fontsize={size}:x=(w-tw)/2:y={y}"


# fframes writes a silent AAC stream; remove it so the deliverable has no audio.
run(['-i', FILM, '-map', '0:v:0', '-c:v', 'copy', '-an', '-movflags', '+faststart', OUT / 'silent.mp4'])
(OUT / 'silent.mp4').replace(FILM)
webm(FILM, OUT / 'originlm-1080p.webm')
run(['-ss', '29', '-i', FILM, '-frames:v', '1', '-q:v', '2', OUT / 'poster.jpg'])
# Recompose the actual dashboard chapter for portrait rather than cropping its text.
portrait = ','.join([
    'crop=1680:700:120:230', 'scale=1080:450', 'pad=1080:1920:0:640:color=0x1e1a1b',
    text('Know all your AI.', 320, 90), text('Usage. Quotas. Reset times.', 1170, 48),
    text('Cost estimates labeled clearly.', 1250, 44), text('OriginLM', 1580, 72, 'eba31a'),
    text('Example data', 1720, 32),
])
run(['-ss', '3', '-t', '7', '-i', FILM, '-vf', portrait, *H264, '-an', OUT / 'see-your-ai-portrait.mp4'])
# The whole camera composition stays intact inside the square cut.
square = ','.join(['scale=1080:608', 'pad=1080:1080:0:210:color=0x1e1a1b', text('OriginLM', 70, 64, 'eba31a'), text('Your app. Your users’ own AI.', 900, 46)])
run(['-ss', '10', '-t', '9', '-i', FILM, '-vf', square, *H264, '-an', OUT / 'build-with-it-square.mp4'])
for stem in ('see-your-ai-portrait', 'build-with-it-square'):
    webm(OUT / f'{stem}.mp4', OUT / f'{stem}.webm')
# A time-reversed hold makes both boundaries continuous with identical end poses.
# The moving glint retraces its edge; there are no dots or jump back to the macro.
run(['-ss', '3', '-t', '3', '-i', OUT / 'studio.mp4', '-filter_complex',
     '[0:v]split[a][b];[b]reverse[r];[a][r]concat=n=2:v=1:a=0[v]', '-map', '[v]', *H264, '-an', OUT / 'hero-loop.mp4'])
webm(OUT / 'hero-loop.mp4', OUT / 'hero-loop.webm')
(OUT / 'stills').mkdir(exist_ok=True)
run(['-i', FILM, '-vf', r'select=not(mod(n\,30))', '-fps_mode', 'vfr', OUT / 'stills/%02d.png'])
print('Encoded main WebM, poster, portrait/square cuts, loop and one-second stills under', OUT)
