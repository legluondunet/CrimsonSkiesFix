# Frozen controls artwork

The owner validated the dev.14 appearance. Do not regenerate or replace it casually.

Expected controls-art.pack: 2555973 bytes, SHA-256 79e6ac7529f3234dd364d752ff5405915d6daf537f5114cc4c1751813a0e6044.

Import the companion generated-art source archive into this directory. Keep controls-art.pack, build_art.py, controllers.png and reference.png; generated panel PNGs may also be retained as editable outputs. All are original generated illustrations, not retail game resources. The actual original game background and logo are sampled only in memory at runtime. Run `python ../../tools/append_controls_art.py --verify-pack controls-art.pack` to validate the frozen pack. To reproduce the graphics, use Python 3 with Pillow and numpy and run build_art.py in this directory; exact compressed output can depend on library versions. CI uses the checked-in authenticated pack rather than regenerating it.
