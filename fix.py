import os

def modify_file():
    with open('docs/API_SPEC.md', 'r', encoding='utf-8') as f:
        content = f.read()

    # remaining json fields
    content = content.replace('"bmdc":', '"license_number":')
    
    # login endpoint description
    content = content.replace(
        '- **Purpose:** Initiates doctor login using BMDC credentials and password (Step 1 of MFA).',
        '- **Purpose:** Initiates doctor login using Medical license credentials + password (Step 1 of MFA). Note: Keep UI labels as "BMDC Registration Number" for Bangladesh deployment. The /bmdc/ path is kept as a V1 Bangladesh-specific path.'
    )
    
    # other mentions
    content = content.replace('BMDC-verified', 'Medical license-verified')

    with open('docs/API_SPEC.md', 'w', encoding='utf-8') as f:
        f.write(content)

if __name__ == '__main__':
    modify_file()
