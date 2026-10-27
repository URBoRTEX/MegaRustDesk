// Browser fixture of the actual panel/TIS. Sciter layout is translated to CSS flex;
// this supplements native Sciter tests, and does not prove the full native viewer.
const fs = require('fs');
const path = require('path');
const assert = require('assert/strict');
const { chromium } = require('playwright');
const root = path.resolve(__dirname, '../..');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');
const panel = read('src/ui/remote.html').split('<section #agent-panel>')[1].split('</section>')[0]
  .replace(/#([\w-]+)/g, 'id="$1"').replace(/<([\w]+)([^>]*?)\s\.([\w-]+)\s\.([\w-]+)/g, '<$1$2 class="$3 $4"')
  .replace(/<([\w]+)([^>]*?)\s\.([\w-]+)/g, '<$1$2 class="$3"').replace(/<div([^>]*?)\s*\/>/g, '<div$1></div>');
const css = (read('src/ui/common.css') + read('src/ui/remote.css'))
  .replace(/var\(([\w-]+)\)\s*:/g, '--$1:').replace(/color\(([\w-]+)\)/g, 'var(--$1)')
  .replace(/flow:\s*horizontal/g, 'display:flex;flex-direction:row').replace(/flow:\s*vertical/g, 'display:flex;flex-direction:column')
  .replace(/border-spacing:/g, 'gap:').replace(/width:\s*\*/g, 'width:100%').replace(/height:\s*\*/g, 'height:100%')
  .replace(/size:\s*\*/g, 'flex:1').replace(/overflow:\s*scroll-indicator/g, 'overflow:hidden');
const stub = read('tools/agent/test_viewer_controls.rs').split('let stub=r#"')[1].split('"#;')[0];
const bridge = `
  const wrapped = new WeakMap();
  function wrap(el) {
    if(!el)return null;if(wrapped.has(el))return wrapped.get(el);
    const value={style:el.style,attributes:new Proxy({}, {get:(_,k)=>el.getAttribute(k),set:(_,k,v)=>{el.setAttribute(k,v);return true;}}),
      state:new Proxy({}, {get:(_,k)=>el[k],set:(_,k,v)=>{el[k]=v;return true;}}),
      on:(name,selector,fn)=>el.addEventListener(name,e=>{const b=e.target.closest(selector);if(b&&el.contains(b))fn.call(wrap(b),e);}),
      timer:(ms,fn)=>{if(ms===1)setTimeout(fn,ms);}};
    Object.defineProperty(value,'text',{get:()=>el.textContent,set:v=>el.textContent=v});wrapped.set(el,value);return value;
  }
  self.select=selector=>wrap(document.querySelector(selector));
  var view={on:function(){},close:function(){},windowMinSize:null};var View={WINDOW_MINIMIZED:1};
`;
const extra = 'html,body{height:100%;overflow:hidden}body{font:16px system-ui}#video-wrapper{flex:1;min-width:0}#agent-panel{flex-shrink:0;box-sizing:content-box}#agent-heading h2{flex:1;width:0}#agent-panel button{box-sizing:border-box;flex-shrink:0}#agent-content{height:auto}';
(async()=>{
  const browser=await chromium.launch({headless:true});const receipts=[];
  try {
    for(const [width,height] of [[1280,800],[900,600],[850,500]])for(const dark of [false,true]) {
      const page=await browser.newPage({viewport:{width,height}});const errors=[];
      page.on('pageerror', e=>errors.push(e.message));
      await page.setContent(`<html class="${dark?'darktheme':''}"><head><meta charset="utf-8"><style>${css}${extra}</style></head><body><div id="video-wrapper"></div><section id="agent-panel">${panel}</section></body></html>`);
      await page.addScriptTag({content:bridge+stub+read('src/ui/agent.tis')});
      const toggle=page.locator('#agent-start'),paste=page.locator('#agent-paste');
      assert.equal(await toggle.textContent(),'Передать Codex');assert.equal(await paste.isEnabled(),true);
      await toggle.click();assert.equal(await toggle.textContent(),'Вернуть управление');assert.equal(await paste.isDisabled(),true);
      const before=await page.locator('#video-wrapper').boundingBox();
      await page.locator('#agent-collapse').click();assert.equal(await page.locator('#agent-content').isVisible(),false);
      const after=await page.locator('#video-wrapper').boundingBox();assert(after.width-before.width>=190);
      await page.locator('#agent-reclaim').click();assert.equal(await page.locator('#agent-reclaim').isVisible(),false);
      await page.locator('#agent-collapse').click();assert.equal(await toggle.textContent(),'Передать Codex');
      await paste.click();assert.equal(await page.evaluate(()=>pasted),1);
      await toggle.focus();await page.keyboard.press('Enter');assert.equal(await page.evaluate(()=>state.mode),'Running');
      await page.keyboard.press('Enter');assert.equal(await page.evaluate(()=>state.mode),'Manual');
      await page.evaluate(()=>{state.manualPasteAllowed=false;updateAgent();});assert.equal(await paste.isDisabled(),true);
      await page.evaluate(()=>{state.connected=false;updateAgent();});assert.equal(await toggle.isDisabled(),true);
      await page.evaluate(()=>{state.connected=true;state.mode='Stopped';updateAgent();});assert.equal(await toggle.isEnabled(),true);
      await toggle.click();assert.equal(await page.evaluate(()=>state.mode),'Manual');
      await page.evaluate(()=>{agentLastError='Ошибка '.repeat(60);state.peer='1'.repeat(180);updateAgent();});
      const overflow=await page.locator('#agent-panel').evaluate(el=>({scroll:el.scrollWidth,client:el.clientWidth}));
      assert(overflow.scroll<=overflow.client,JSON.stringify(overflow));assert.deepEqual(errors,[]);
      // Save two synthetic-only views, never the real remote desktop.
      if(width===900) {
        await page.evaluate(()=>{agentLastError='';state.peer='fixture';state.manualPasteAllowed=true;state.status='Вы управляете';updateAgent();});
        if(dark)await page.locator('#agent-collapse').click();
        await page.screenshot({path:path.join(root,`.agent-build/viewer-controls-${width}-${dark?'dark':'light'}.png`)});
      }
      receipts.push({width,height,dark,pass:true,overflow,consoleErrors:errors});await page.close();
    }
    fs.writeFileSync(path.join(root,'.agent-build/viewer-controls-browser.json'),JSON.stringify({fixture:true,fullNativeViewer:false,receipts},null,2));
    console.log(JSON.stringify(receipts));
  } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
