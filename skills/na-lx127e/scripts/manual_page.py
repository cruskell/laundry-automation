#!/usr/bin/env python3
"""Read or render a page from the manuals bundled with this skill."""
import argparse
from pathlib import Path
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('document', choices=['manual', 'guide'])
    parser.add_argument('page', type=int, help='One-based PDF page, not printed page')
    parser.add_argument('--render', type=Path, help='Write this page as a PNG for visual inspection')
    args = parser.parse_args()
    try:
        import pymupdf
    except ImportError:
        parser.error('PyMuPDF is required. Install it in a temporary environment: pip install pymupdf')
    target = {'manual': '洗濯機_取扱説明書_NA-LX127E.pdf',
              'guide': '洗濯機_ご使用ガイド_NA-LX127E.pdf'}[args.document]
    source = Path(__file__).resolve().parents[1] / 'references' / 'manuals' / target
    if not source.is_file():
        parser.error(f'Bundled manual missing: {source}')
    with pymupdf.open(source) as document:
        if not 1 <= args.page <= len(document):
            parser.error(f'PDF page must be in 1..{len(document)}')
        page = document[args.page - 1]
        print(f'{source.name} | PDF page {args.page}/{len(document)}')
        print(page.get_text())
        if args.render:
            if args.render.suffix.lower() != '.png':
                parser.error('--render requires a .png path')
            page.get_pixmap(matrix=pymupdf.Matrix(1.5, 1.5)).save(args.render)
            print(f'Rendered: {args.render}', file=sys.stderr)


if __name__ == '__main__':
    main()
