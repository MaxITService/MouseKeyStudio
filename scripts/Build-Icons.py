"""Encode the original orange mouse artwork for the app and Windows tray.

Run with Python and Pillow. The master was generated for this distribution
on 2026-11-03; resizing preserves its colors and transparency.
"""

from pathlib import Path

from PIL import Image


def main():
    root = Path(__file__).resolve().parent.parent
    assets = root / "assets/plus"
    source = Image.open(assets / "icon-source.webp").convert("RGBA")
    assert source.width == source.height
    assert source.getchannel("A").getextrema() == (0, 255)
    app = source.resize((1024, 1024), Image.Resampling.LANCZOS)
    app.save(assets / "icon.png")
    sizes = [(size, size) for size in (16, 24, 32, 48, 64, 128, 256)]
    source.save(assets / "icon.ico", sizes=sizes)
    for bundle in ("studio.icon", "studio-prism.icon"):
        app.save(assets / bundle / "Assets/studio.png")
    tray = root / "crates/openlogi-agent/assets/tray-icon-orange@2x.png"
    source.resize((36, 36), Image.Resampling.LANCZOS).save(tray)
    ico = Image.open(assets / "icon.ico")
    assert ico.ico.sizes() == set(sizes)
    for size in sizes:
        actual = ico.ico.getimage(size).convert("RGBA")
        expected = source.resize(size, Image.Resampling.LANCZOS)
        assert actual.tobytes() == expected.tobytes(), size
    assert Image.open(tray).tobytes() == source.resize(
        (36, 36), Image.Resampling.LANCZOS
    ).tobytes()
    print("PASS: app ICO frames and tray use the same orange mouse master")


if __name__ == "__main__":
    main()
