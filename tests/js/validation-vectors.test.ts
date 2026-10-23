import { readFileSync } from 'node:fs'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const tauriCoreMock = vi.hoisted(() => ({
	invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => tauriCoreMock)

interface Vectors {
	urls: { url: string; valid: boolean }[]
	filePaths: { path: string; posixValid: boolean }[]
}

const vectors: Vectors = JSON.parse(
	readFileSync(new URL('../fixtures/validation-vectors.json', import.meta.url), 'utf8')
)

describe('shared validation vectors', () => {
	beforeEach(() => {
		tauriCoreMock.invoke.mockReset()
		tauriCoreMock.invoke.mockResolvedValue({ value: true })
	})

	it.each(vectors.urls)('url $url is valid: $valid', async ({ url, valid }) => {
		const { canShare } = await import('../../guest-js/index')

		await expect(canShare({ url })).resolves.toBe(valid)
	})

	it.each(vectors.filePaths)('file path $path is valid: $posixValid', async ({ path, posixValid }) => {
		const { canShare } = await import('../../guest-js/index')

		await expect(canShare({ filePaths: [path] })).resolves.toBe(posixValid)
	})
})

describe('payload normalization', () => {
	beforeEach(() => {
		tauriCoreMock.invoke.mockReset()
		tauriCoreMock.invoke.mockResolvedValue(undefined)
	})

	it('drops empty text, title, subject and url before invoking share', async () => {
		const { share } = await import('../../guest-js/index')

		await share({ text: 'hello', title: '', subject: '', url: '' })

		const [, args] = tauriCoreMock.invoke.mock.calls[0]
		expect(args.options).toStrictEqual({
			text: 'hello',
			title: undefined,
			subject: undefined,
			url: undefined,
		})
	})

	it('rejects non-finite anchor values', async () => {
		const { share } = await import('../../guest-js/index')

		await expect(
			share({ text: 'hello', anchor: { x: Number.NaN, y: 0, width: 1, height: 1 } })
		).rejects.toThrow('anchor values must be finite numbers.')
		expect(tauriCoreMock.invoke).not.toHaveBeenCalled()
	})
})
