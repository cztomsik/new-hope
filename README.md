Basic PoC just to get some idea about perf of my iGPU & Raspis
it is intentionally dumb & many things are missing:
- separate opaque & alpha passes
- Z-sorting
- culling
- clipping
- basically it's just a classic painter algo for now

there's also no scene, it's a kind of imgui but even without any
event handling (nor hit-testing), no layout, no anything

things missing but in a scope of this PoC:
- text rendering & atlasing
- images (no loading/decoding, just generate some checkboard)
- round border
- blur shadow (now it's just an outline)
- fill round rect (with different corner radiis)

the idea is to get something working on osx, raspi & in a browser
improve it a bit and then port it back to the original project
as a replacement for webrender

## Results
- raspi zero w chokes even with basic LXDE environment so I'm not going to even try
- raspi 3 A+ runs fine, around 350 fps
- raspi 4 runs fine, need to retest without vsync
- 2015 macbook air is around 450 fps 

## Build

### osx & raspi:
```
cargo run --example main
```

### emscripten (TODO):
```
brew install emscripten
cargo rustc --target=wasm32-unknown-emscripten -- -Clink-arg='-s' -Clink-arg='USE_SDL=2'
http-server
open http://127.0.0.1:8080/test.html
```
