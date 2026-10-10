// Run this function in the local Playwright browser session after starting auth_bridge.py.
async (page) => {
  const config = await (await page.request.get('http://127.0.0.1:18765/start')).json();
  page.on('response', async (response) => {
    try {
      const location = (await response.allHeaders()).location;
      if (location && location.startsWith(config.redirect)) {
        await page.request.post('http://127.0.0.1:18765/callback', { data: { location } });
      }
    } catch (error) {
      // The bridge records token exchange failures without exposing credentials.
    }
  });
  await page.goto(config.url);
  return { loginPageOpened: true };
};
