import './main.css'
;(async () => {
  const baseUrl = ''
  const temp = document.querySelector('#summary-temp') as HTMLTemplateElement
  const container = document.querySelector('.summaries')

  if (!container || !temp) {
    console.warn('Cannot find summary container or list template ')
    return
  }

  await fetch(`${baseUrl}/feeds`)
    .then(x => x.json() as Promise<{ items: FeedSummary[] }>)
    .then(x => {
      x.items.forEach(x => {
        const node = document.importNode(temp.content, true)
        const datetime = new Date(x.create_at).toLocaleDateString(undefined, {
          weekday: 'short',
          year: '2-digit',
          month: '2-digit',
          hour: '2-digit',
          minute: '2-digit'
        })

        ;(
          node.querySelector('.summary') as HTMLAnchorElement
        ).href = `${baseUrl}/feeds/${x.id}`

        node.querySelector('.summary-id').textContent = '#' + x.id
        node.querySelector('.summary-title').textContent = x.title
        node.querySelector('.summary-date').textContent = datetime
        container.appendChild(node)
      })
    })
})()

interface FeedSummary {
  title: string
  create_at: string
  id: string
}
