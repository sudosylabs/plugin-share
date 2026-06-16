import { canShare, cleanup, share, type ShareData } from '@vnidrop/tauri-plugin-share'

async function checkShareImports(file: File): Promise<void> {
	const data: ShareData = {
		title: 'Report',
		text: 'Share this report',
		url: 'https://example.com/report',
		files: [file],
	}

	const supported: boolean = await canShare(data)
	if (supported) {
		await share(data)
	}
	await cleanup()
}

void checkShareImports
