'use strict';

const path = require('node:path');
const { app, BrowserWindow } = require('electron');

app.enableSandbox();
let fixtureWindow;

async function openFixture() {
  fixtureWindow = new BrowserWindow({
    width: 760,
    height: 640,
    show: false,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      spellcheck: false,
      partition: 'p05c-fixture',
    },
  });

  const contents = fixtureWindow.webContents;
  contents.setWindowOpenHandler(() => ({ action: 'deny' }));
  contents.on('will-navigate', (event) => event.preventDefault());
  await fixtureWindow.loadFile(path.join(__dirname, 'input.html'));
  fixtureWindow.show();
}

app.whenReady().then(openFixture).catch(() => {
  process.stderr.write('P05C fixture window or local page failed to load.\n');
  app.exit(1);
});
app.on('window-all-closed', () => app.quit());
