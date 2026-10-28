"""Approved 2026-10-25 branding contract. Requires Pillow; does not modify files."""
from pathlib import Path
from hashlib import sha256
import json
import unittest
from PIL import Image
ROOT = Path(__file__).resolve().parents[1]
APP = 'ModelStrike'
APPROVED = '3d6338d9e314369ffe04da23036ef923684f53377e350a36fcd910d26d6f6d83'
class ApprovedBranding(unittest.TestCase):
    def test_exact_approved_source_and_readme(self):
        p = ROOT / 'assets/branding' / (APP + '.png')
        self.assertEqual(sha256(p.read_bytes()).hexdigest(), APPROVED)
        with Image.open(p) as image:
            self.assertEqual(image.size, (512,512))
        website = ROOT / 'docs/branding/ModelStrike-website.webp'
        self.assertEqual(sha256(website.read_bytes()).hexdigest(), '037027d2122253231dceef3b46ae2b34a635c56515d88073dbd920709a4ee152')
        with Image.open(website) as image:
            self.assertEqual(image.size, (512, 512))
        self.assertIn('src="docs/branding/ModelStrike-website.webp"', (ROOT/'README.md').read_text(encoding="utf-8"))
        self.assertEqual(json.loads((p.parent/'provenance.json').read_text(encoding="utf-8"))['sha256'], APPROVED)
    def test_runtime_derivative_and_binding(self):
        source = Image.open(ROOT/'assets/branding'/(APP+'.png')).convert('RGBA')
        raw = ROOT/'assets/branding'/(APP+'-128.rgba')
        self.assertTrue(raw.exists(), 'runtime derivative absent')
        self.assertEqual(raw.read_bytes(), source.resize((128,128),Image.Resampling.LANCZOS).tobytes())
        self.assertIn(APP+'-128.rgba', (ROOT/'src/bin/modelstrike-ui.rs').read_text(encoding="utf-8"))

if __name__ == "__main__":
    unittest.main()
