"""Copy canonical font assets into the project; never install fonts on Windows."""
import asyncio
import re
from pathlib import Path
from playwright.async_api import async_playwright
ROOT=Path(__file__).resolve().parents[3]
async def main():
    source=(Path(__file__).parent/'reference/font-source.css').read_text(encoding='utf-8')
    out=ROOT/'apps/doctor-web/public/fonts';out.mkdir(parents=True,exist_ok=True)
    rules=[]
    async with async_playwright() as pw:
        context=await pw.request.new_context()
        for block in re.findall(r'@font-face\s*\{([^}]+)\}',source):
            family=re.search(r"font-family: '([^']+)'",block)[1]
            weight=re.search(r'font-weight: (\d+)',block)[1]
            url=re.search(r'url\(([^)]+)\)',block)[1]
            file=('plus-jakarta-sans' if family=='Plus Jakarta Sans' else 'inter')+'-'+weight+'.ttf'
            response=await context.get(url)
            if not response.ok:raise RuntimeError('Canonical font unavailable')
            (out/file).write_bytes(await response.body())
            rules.append(f"@font-face {{ font-family: '{family}'; font-style: normal; font-weight: {weight}; font-display: swap; src: url('/fonts/{file}') format('truetype'); }}")
        await context.dispose()
    (ROOT/'apps/doctor-web/src/app/fonts.css').write_text('\n'.join(rules)+'\n',encoding='utf-8')
    print('Saved',len(rules),'canonical font assets in the project.')
asyncio.run(main())
