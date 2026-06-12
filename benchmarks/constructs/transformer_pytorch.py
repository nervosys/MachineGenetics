import torch
import torch.nn as nn

class TransformerBlock(nn.Module):
    def __init__(self, d=256, heads=8, ff=1024):
        super().__init__()
        self.attn = nn.MultiheadAttention(d, heads, batch_first=True)
        self.norm1 = nn.LayerNorm(d)
        self.ff = nn.Sequential(nn.Linear(d, ff), nn.GELU(), nn.Linear(ff, d))
        self.norm2 = nn.LayerNorm(d)

    def forward(self, x):
        a, _ = self.attn(x, x, x)
        x = self.norm1(x + a)
        return self.norm2(x + self.ff(x))
