import './main.css'

const baseUrl = ''

;(async () => {
  fetch(`${baseUrl}/feeds`)
    .then(x => x.json() as Promise<{ items: FeedSummary[] }>)
    .then(x => {
      const temp = document.querySelector(
        '#summary-temp'
      ) as HTMLTemplateElement
      const container = document.querySelector('.summaries')
      if (!container || !temp) {
        console.warn('Cannot find summary container or list template ')
        return
      }
      x.items.forEach(x => {
        const node = document.importNode(temp.content, true)

        ;(
          node.querySelector('.summary') as HTMLAnchorElement
        ).href = `${baseUrl}/feeds/${x.id}`

        node.querySelector('.summary-id').textContent = '#' + x.id
        node.querySelector('.summary-title').textContent = x.title
        node.querySelector('.summary-date').textContent = new Date(
          x.create_at
        ).toDateString()
        container.appendChild(node)
      })
    })
})()

interface FeedSummary {
  title: string
  create_at: string
  id: string
}
