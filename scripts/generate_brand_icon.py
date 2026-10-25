"""Resize the full approved canvas, retaining proportions, centering and padding."""
from pathlib import Path
from PIL import Image
ROOT = Path(__file__).resolve().parents[1]
APP = 'ModelStrike'
image = Image.open(ROOT/'assets/branding'/(APP+'.png')).convert('RGBA')
(ROOT/'assets/branding'/(APP+'-128.rgba')).write_bytes(image.resize((128,128),Image.Resampling.LANCZOS).tobytes())
