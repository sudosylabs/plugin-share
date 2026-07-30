import { Buffer } from 'node:buffer'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const tauriCoreMock = vi.hoisted(() => ({
	invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => tauriCoreMock)

class TestFileReader {
	result: string | ArrayBuffer | null = null
	onload: (() => void) | null = null
	onerror: ((error: unknown) => void) | null = null

	readAsDataURL(file: Blob) {
		file.arrayBuffer()
			.then(buffer => {
				const base64 = Buffer.from(buffer).toString('base64')
				this.result = `data:${file.type};base64,${base64}`
				this.onload?.()
			})
			.catch(error => this.onerror?.(error))
	}
}

describe('share guest API', () => {
	beforeEach(() => {
		tauriCoreMock.invoke.mockReset()
		tauriCoreMock.invoke.mockResolvedValue(undefined)
		vi.stubGlobal('FileReader', TestFileReader)
	})

	it('checks native share capability through the plugin command', async () => {
		tauriCoreMock.invoke.mockResolvedValue({ value: true })
		const { canShare } = await import('../../guest-js/index')

		await expect(canShare()).resolves.toBe(true)

		expect(tauriCoreMock.invoke).toHaveBeenCalledWith('plugin:vnidrop-share|can_share')
	})

	it('returns false for invalid data without invoking native code', async () => {
		const { canShare } = await import('../../guest-js/index')

		await expect(canShare({ url: 'file:///tmp/secret.txt' })).resolves.toBe(false)

		expect(tauriCoreMock.invoke).not.toHaveBeenCalled()
	})

	it('converts File payloads to base64 before invoking share', async () => {
		const { share } = await import('../../guest-js/index')
		const file = new File(['hello'], 'hello.txt', { type: 'text/plain' })

		await share({
			title: 'Greeting',
			text: 'Share this',
			url: 'https://example.com',
			files: [file],
		})

		expect(tauriCoreMock.invoke).toHaveBeenCalledWith('plugin:vnidrop-share|share', {
			options: {
				title: 'Greeting',
				text: 'Share this',
				url: 'https://example.com',
				files: [
					{
						data: 'aGVsbG8=',
						name: 'hello.txt',
						mimeType: 'text/plain',
					},
				],
			},
		})
	})

	it('passes an anchor option to the native share command', async () => {
		const { share } = await import('../../guest-js/index')

		await share({
			title: 'Greeting',
			text: 'Share this',
			url: 'https://example.com',
			anchor: { x: 10, y: 20, width: 120, height: 44 },
		})

		expect(tauriCoreMock.invoke).toHaveBeenCalledWith('plugin:vnidrop-share|share', {
			options: {
				title: 'Greeting',
				text: 'Share this',
				url: 'https://example.com',
				anchor: { x: 10, y: 20, width: 120, height: 44 },
			},
		})
	})

	it('rejects non-web url schemes in the url field', async () => {
		const { share } = await import('../../guest-js/index')

		await expect(share({ url: 'file:///tmp/secret.txt' })).rejects.toThrow(
			'Only http:// and https:// URLs can be shared as URLs.'
		)

		expect(tauriCoreMock.invoke).not.toHaveBeenCalled()
	})

	it('rejects malformed web urls before invoking native code', async () => {
		const { share } = await import('../../guest-js/index')

		await expect(share({ url: 'https:///missing-host' })).rejects.toThrow(
			'Only http:// and https:// URLs can be shared as URLs.'
		)
		await expect(share({ url: 'https://example.com\nhttps://evil.example' })).rejects.toThrow(
			'Only http:// and https:// URLs can be shared as URLs.'
		)

		expect(tauriCoreMock.invoke).not.toHaveBeenCalled()
	})

	it('rejects oversized file metadata before invoking native code', async () => {
		const { share } = await import('../../guest-js/index')
		const file = new File(['hello'], 'a'.repeat(256), { type: 'text/plain' })

		await expect(share({ files: [file] })).rejects.toThrow(
			'File name exceeds the maximum length of 255 bytes.'
		)

		expect(tauriCoreMock.invoke).not.toHaveBeenCalled()
	})

	it('keeps cleanup as an explicit command', async () => {
		const { cleanup } = await import('../../guest-js/index')

		await cleanup()

		expect(tauriCoreMock.invoke).toHaveBeenCalledWith('plugin:vnidrop-share|cleanup')
	})
})
