import os
import re

base_dir = r"c:\Users\CUBE\Desktop\helodoc old site"

files = [
    "BUSINESS_LOGIC.md",
    r"docs\USER_FLOWS.md",
    r"docs\INTERACTION_SPEC.md",
    r"docs\ERROR_HANDLING.md",
    r"docs\VALIDATION_RULES.md"
]

for f in files:
    filepath = os.path.join(base_dir, f)
    with open(filepath, 'r', encoding='utf-8') as file:
        content = file.read()
    
    content = re.sub(r'ESCROW_HELD', 'PAYMENT_HELD', content)
    content = re.sub(r'ESCROW_RELEASED', 'PAYMENT_RELEASED', content)
    content = re.sub(r'ESCROW_REFUNDED', 'PAYMENT_REFUNDED', content)
    content = re.sub(r'(?i)escrow', 'payment hold', content)
    
    with open(filepath, 'w', encoding='utf-8') as file:
        file.write(content)

print("Done part 3")
