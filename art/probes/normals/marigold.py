import os, sys, time, torch, numpy as np, diffusers
from PIL import Image
OUT = os.environ.get("OUT", "outA")
dev = "cuda" if torch.cuda.is_available() and not os.environ.get("CPU") else "cpu"
dt = torch.float16 if dev == "cuda" else torch.float32
t0 = time.time()
pipe = diffusers.MarigoldNormalsPipeline.from_pretrained("prs-eth/marigold-normals-v1-1", variant="fp16", torch_dtype=dt).to(dev)
pipe.enable_attention_slicing()
print("load", dev, round(time.time()-t0,1), flush=True)
for p in sys.argv[4:]:
    img = Image.open(p).convert("RGB")
    t0 = time.time()
    out = pipe(img, num_inference_steps=int(sys.argv[2]), ensemble_size=int(sys.argv[3]), processing_resolution=int(sys.argv[1]), output_type="np")
    n = np.asarray(out.prediction[0], dtype=np.float32)
    name = p.split("/")[-1][:-4]
    np.save(f"{OUT}/{name}.npy", n)
    Image.fromarray(((n+1)*127.5).clip(0,255).astype(np.uint8)).save(f"{OUT}/{name}-n.png")
    print(name, n.shape, "s", round(time.time()-t0,1), "peakMB", torch.cuda.max_memory_allocated()//2**20 if dev=="cuda" else 0, flush=True)
