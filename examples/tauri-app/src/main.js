import { canShare, cleanup, share } from '@vnidrop/tauri-plugin-share'
import './style.css'

const app = document.querySelector('#app')

app.innerHTML = `
  <section class="workspace">
    <header class="toolbar">
      <div>
        <h1>Vnidrop Share</h1>
        <p id="support">Checking native share support...</p>
      </div>
      <button id="cleanup" type="button">Cleanup Temp Files</button>
    </header>

    <form id="share-form" class="panel">
      <label>
        Title
        <input id="title" name="title" value="Vnidrop Share Example" />
      </label>

      <label>
        Text
        <textarea id="text" name="text" rows="4">Sharing from a Tauri app with @vnidrop/tauri-plugin-share.</textarea>
      </label>

      <label>
        Web URL
        <input id="url" name="url" type="url" value="https://github.com/vnidrop/plugin-share" />
      </label>

      <label>
        Local files
        <input id="files" name="files" type="file" multiple />
      </label>

      <div class="actions">
        <button id="share-text" type="submit">Share Content</button>
        <button id="share-file" type="button">Share Selected Files</button>
      </div>
    </form>

    <output id="status" class="status">Ready</output>
  </section>
`

const support = document.querySelector('#support')
const status = document.querySelector('#status')
const form = document.querySelector('#share-form')
const fileInput = document.querySelector('#files')

function setStatus(message) {
	status.textContent = message
}

async function refreshSupport() {
	const supported = await canShare()
	support.textContent = supported ? 'Native sharing is available.' : 'Native sharing is unavailable on this platform.'
}

form.addEventListener('submit', async event => {
	event.preventDefault()
	const data = new FormData(form)

	try {
		await share({
			title: String(data.get('title') || ''),
			text: String(data.get('text') || ''),
			url: String(data.get('url') || ''),
		})
		setStatus('Share sheet closed.')
	} catch (error) {
		setStatus(error instanceof Error ? error.message : String(error))
	}
})

document.querySelector('#share-file').addEventListener('click', async () => {
	const files = Array.from(fileInput.files || [])
	if (files.length === 0) {
		setStatus('Select at least one local file first.')
		return
	}

	try {
		await share({
			title: 'Selected local files',
			text: 'Sharing local file content through temporary native files.',
			files,
		})
		setStatus(`Shared ${files.length} file${files.length === 1 ? '' : 's'}.`)
	} catch (error) {
		setStatus(error instanceof Error ? error.message : String(error))
	}
})

document.querySelector('#cleanup').addEventListener('click', async () => {
	try {
		await cleanup()
		setStatus('Temporary files cleaned up.')
	} catch (error) {
		setStatus(error instanceof Error ? error.message : String(error))
	}
})

refreshSupport().catch(error => {
	support.textContent = error instanceof Error ? error.message : String(error)
})
