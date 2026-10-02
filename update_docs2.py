import os
import re

base_dir = r"c:\Users\CUBE\Desktop\helodoc old site"

def update_file(filepath, replacements):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    for old, new in replacements:
        content = re.sub(old, new, content, flags=re.MULTILINE|re.IGNORECASE)
        
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(content)

# BUSINESS_LOGIC.md
bl_path = os.path.join(base_dir, "BUSINESS_LOGIC.md")
bl_replacements = [
    (r"WebRTC", "Agora"),
    (r"webrtc", "agora")
]
update_file(bl_path, bl_replacements)

# USER_FLOWS.md
uf_path = os.path.join(base_dir, "docs", "USER_FLOWS.md")
uf_replacements = [
    (r"WEBRTC_SIGNALING", "AGORA_RTC")
]
update_file(uf_path, uf_replacements)

print("Done part 2")
