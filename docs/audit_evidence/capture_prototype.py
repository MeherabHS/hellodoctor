"""Capture the canonical Doctor HTML, without modifying the source file."""
import asyncio
import json
from pathlib import Path
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "prototype" / "index.html"
OUT = Path(__file__).resolve().parent / "prototype"
SCREENS = {"queue": 0, "chat": 4, "smart_rx": 1, "schedule": 2, "wallet": 3, "qna": 5}


async def main():
    OUT.mkdir(parents=True, exist_ok=True)
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(channel="msedge", headless=True)
        page = await browser.new_page(viewport={"width": 1100, "height": 1100}, device_scale_factor=1)
        await page.goto(SOURCE.as_uri(), wait_until="load")
        await page.evaluate("switchRole('doctor')")
        await page.evaluate("document.fonts.ready")
        measurements = {}
        for width in (360, 390, 412):
            # Strip only the simulated handset frame; original components and CSS remain intact.
            await page.add_style_tag(content=f".phone{{width:{width}px!important;padding:0!important;border-radius:0!important}} .screen-shell{{height:800px!important;border-radius:0!important}}")
            for name, tab in SCREENS.items():
                await page.evaluate("tab => switchDoctorTab(tab)", tab)
                await page.locator(f"#doc-view-{tab}").wait_for(state="visible")
                await page.locator("#appShell").screenshot(path=str(OUT / f"{name}_{width}.png"))
                measurements[f"{name}_{width}"] = await page.locator(f"#doc-view-{tab}").evaluate("""root => [...root.querySelectorAll('.dir-header,.doc-header-card,.conv-card,.rx-scan-hero-card,.doc-slot-card,.doc-earnings-breakdown-card,.flow-top')].map(el => { const s=getComputedStyle(el), r=el.getBoundingClientRect(); return {selector:el.className,width:r.width,height:r.height,padding:s.padding,fontSize:s.fontSize,fontFamily:s.fontFamily,fontWeight:s.fontWeight,radius:s.borderRadius,color:s.color,background:s.backgroundColor}; })""")
        (OUT / "computed_styles.json").write_text(json.dumps(measurements, indent=2), encoding="utf-8")
        await browser.close()
    print(f"Captured {len(SCREENS) * 3} canonical Doctor screens in {OUT}")


if __name__ == "__main__":
    asyncio.run(main())
