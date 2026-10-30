// Install a test provider into a generated, isolated debug Gradle project, not the repository.
// node scripts/android/theme-import/install-fixture.cjs <generated app/src/main>
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const {createHash} = require('node:crypto');
const root = path.resolve(__dirname, '../../..');
const main = fs.realpathSync(path.resolve(process.argv[2]));
const relative = path.relative(path.join(root, 'target'), main);
assert.ok(relative && !relative.startsWith('..') && !path.isAbsolute(relative), 'generated target only');
const gradle = fs.readFileSync(path.join(main, '../../build.gradle.kts'), 'utf8');
assert.match(gradle, /applicationId\s*=\s*"io\.github\.develata\.rssreader\.backpolicytest"/);
const file = path.join(main, 'AndroidManifest.xml');
const manifest = fs.readFileSync(file, 'utf8');
assert.ok(!manifest.includes('ThemeImportDocuments'), 'fresh generated manifest required');
assert.ok(manifest.includes('</application>'));
fs.writeFileSync(file, manifest.replace('</application>', `<provider
    android:name="dev.dioxus.main.ThemeImportDocuments"
    android:authorities="io.github.develata.rssreader.backpolicytest.themefixtures"
    android:exported="true" android:grantUriPermissions="true"
    android:permission="android.permission.MANAGE_DOCUMENTS">
    <intent-filter><action android:name="android.content.action.DOCUMENTS_PROVIDER" /></intent-filter>
  </provider></application>`));
const source = path.join(__dirname, 'ThemeImportDocuments.kt');
const destination = path.join(main, 'kotlin/dev/dioxus/main/ThemeImportDocuments.kt');
fs.mkdirSync(path.dirname(destination), {recursive: true});
fs.copyFileSync(source, destination);
const sha = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
console.log(JSON.stringify({main,providerSha256:sha(source),manifestSha256:sha(file)}, null, 2));
