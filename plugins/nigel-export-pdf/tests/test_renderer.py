from pathlib import Path

from nigel_export_pdf.renderer import render_report_to_pdf, TEMPLATES_DIR


def test_render_base_template_produces_pdf(tmp_path):
    """Rendering base template with minimal content produces valid PDF."""
    test_template = TEMPLATES_DIR / "_test.html"
    test_template.write_text(
        '{% extends "base.html" %}'
        '{% block content %}<p>Test content</p>{% endblock %}'
    )

    try:
        output = tmp_path / "test.pdf"
        render_report_to_pdf(
            "_test.html",
            {},
            output,
            title="Test Report",
            company_name="Test Co",
            date_range="2025",
        )
        assert output.exists()
        assert output.stat().st_size > 0
        assert output.read_bytes()[:5] == b"%PDF-"
    finally:
        test_template.unlink(missing_ok=True)
