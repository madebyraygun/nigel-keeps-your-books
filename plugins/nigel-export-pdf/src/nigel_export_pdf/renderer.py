from datetime import datetime
from pathlib import Path

from jinja2 import Environment, FileSystemLoader


TEMPLATES_DIR = Path(__file__).parent / "templates"


def render_report_to_pdf(
    template_name: str,
    data: dict,
    output_path: Path,
    title: str = "Report",
    company_name: str | None = None,
    date_range: str | None = None,
) -> Path:
    """Render a report to PDF using a Jinja2 template and WeasyPrint."""
    from weasyprint import HTML

    env = Environment(loader=FileSystemLoader(str(TEMPLATES_DIR)))
    env.filters["abs"] = abs
    env.filters["currency"] = lambda v: f"${abs(v):,.2f}"

    template = env.get_template(template_name)
    html_content = template.render(
        title=title,
        company_name=company_name,
        date_range=date_range,
        generated_at=datetime.now().strftime("%Y-%m-%d %H:%M"),
        **data,
    )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    HTML(string=html_content).write_pdf(str(output_path))
    return output_path
