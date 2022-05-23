import './main.css'

const sleep = (ms: number) =>
  new Promise(res => {
    setTimeout(res, ms)
  })
;(async () => {
  const baseUrl = 'http://localhost:8080'
  const template = document.querySelector('.summary-template') as HTMLTemplateElement | null
  const container = document.querySelector('.summaries')

  if (!container || !template) {
    console.warn('Cannot find summary container or list template ')
    return
  }

  // 5 second timeout:
  const controller = new AbortController()

  const timeoutId = setTimeout(() => controller.abort(), 1000)

  await fetch(`${baseUrl}/feeds`, { signal: controller.signal })
    .then(resp => resp.json() as Promise<{ items: FeedSummary[] }>)
    .then(async feeds => {
      clearTimeout(timeoutId)
      container.querySelector('.summaries-placeholder')?.remove()
      feeds.items.forEach(x => {
        // Clone the template
        const node = document.importNode(template.content, true)

        // Format the date
        const datetime = new Date(x.create_at).toLocaleDateString(undefined, {
          weekday: 'short',
          year: '2-digit',
          month: '2-digit',
          hour: '2-digit',
          minute: '2-digit'
        })

        const [summary, id, title, date] = [
          node.querySelector('.summary') as HTMLAnchorElement | null,
          node.querySelector('.summary-id'),
          node.querySelector('.summary-title'),
          node.querySelector('.summary-date')
        ]

        summary && (summary.href = `${baseUrl}/feeds/${x.id}`)
        id && (id.textContent = '#' + x.id)
        title && (title.textContent = x.title)
        date && (date.textContent = datetime)

        container.appendChild(node)
      })
    }).catch(e => {
      console.warn('Failed to fetch', e)
      let placeholder = container.querySelector('.summaries-placeholder')
      placeholder && (placeholder.textContent = `Failed to fetch: ${e.message}`)
    })
})()

interface FeedSummary {
  title: string
  create_at: string
  id: string
}
