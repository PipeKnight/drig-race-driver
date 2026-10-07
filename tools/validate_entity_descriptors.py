"""Independently resolve original entity records and compare native bit reports."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
from survey_pssg_payloads import survey


class Library:
    def __init__(self, path):
        self.report = survey(path)
        self.bytes = path.read_bytes()
        self.nodes = self.report['nodes']
        self.children = collections.defaultdict(list)
        self.ids = {}
        for i, node in enumerate(self.nodes):
            self.children[node['parent']].append(i)
            if 'id' in node['attributes']:
                name = self.text(i, 'id')
                if name in self.ids:
                    raise ValueError('Duplicate ID')
                self.ids[name] = i

    def text(self, i, name):
        return self.nodes[i]['attributes'][name]['string']

    def raw(self, i, name):
        return self.nodes[i]['attributes'][name]['hex']

    def one(self, i, kind):
        matches = [c for c in self.children[i] if self.nodes[c]['name'] == kind]
        if len(matches) != 1:
            raise ValueError(f'Expected exactly one {kind}')
        return matches[0]

    def matrix(self, i):
        n = self.nodes[self.one(i, 'TEMPLATETRANSFORM')]
        data = self.bytes[n['payload_offset']:n['payload_offset'] + n['payload_size']]
        if len(data) != 64:
            raise ValueError('Invalid transform size')
        return ','.join(data[j:j+4].hex() for j in range(0, 64, 4))

    def local(self, uri):
        if not uri.startswith('#'):
            raise ValueError('Expected local URI')
        return self.ids[uri[1:]]


def rows(root):
    p = Library(root/'tracks/europe/donington/route_0/objects.ens')
    t = Library(root/'tracks/europe/donington/objecttypes.pssg')
    result = []
    for i, node in enumerate(p.nodes):
        if node['name'] != 'TEMPLATEENTITYINSTANCE':
            continue
        reference = p.local(p.text(i, 'uri'))
        uri = p.text(reference, 'uri')
        assert uri.startswith('objecttypes.pssg#')
        template_id = uri.split('#', 1)[1]
        template = t.ids[template_id]
        behaviour = t.local(t.text(t.one(template, 'TEMPLATEBEHAVIOUR'), 'typeuri'))
        active = set()
        for c in t.children[behaviour]:
            assert t.nodes[c]['name'] == 'TEMPLATECOMPONENTCHANGE'
            target = t.local(t.text(c, 'componenturi'))
            assert t.nodes[target]['parent'] == template
            operation = t.text(c, 'operation')
            if operation == 'add':
                active.add(target)
            elif operation == 'delete':
                active.discard(target)
            else:
                raise ValueError('Unknown component operation')
        name = p.text(i, 'id')
        links = sum(p.nodes[c]['name'] == 'TEMPLATEENTITYLINK' for c in p.children[i])
        result.append(['E', name, template_id, p.matrix(i), str(links)])
        for component in sorted(active):
            cid = t.text(component, 'id')
            if t.nodes[component]['name'] == 'TEMPLATECLOTH':
                result.append(['C', name, cid])
                continue
            assert t.nodes[component]['name'] == 'TEMPLATESUBENTITY'
            transformable = t.one(component, 'TEMPLATETRANSFORMABLE')
            renderable = t.local(t.text(transformable, 'uri'))
            body = t.one(component, 'TEMPLATERIGIDBODY')
            mass = t.nodes[body]['attributes']['infMass']['u32be']
            assert mass in (0, 1)
            result.append(['R', name, cid, t.text(renderable, 'uri'), t.text(transformable, 'sid'),
                           t.text(body, 'id'), t.text(body, 'props'), str(bool(mass)).lower()])
            for s in t.children[body]:
                if t.nodes[s]['name'] != 'TEMPLATERIGIDBODYSHAPE':
                    assert t.nodes[s]['name'] == 'TEMPLATELINKPOINT'
                    continue
                shape = t.local(t.text(s, 'uri'))
                kind = t.nodes[shape]['name']
                if kind == 'TEMPLATESHAPEBOX':
                    primitive, dimensions = 'box', ['width', 'height', 'length']
                elif kind == 'TEMPLATESHAPECYLINDER':
                    primitive, dimensions = 'cylinder', ['radius', 'height']
                else:
                    raise ValueError('Unknown collision shape')
                phantom = t.nodes[shape]['attributes']['phantom']['u32be']
                assert phantom in (0, 1)
                result.append(['S', name, cid, t.text(shape, 'id'), primitive,
                               ','.join(t.raw(shape, d) for d in dimensions), t.matrix(s),
                               t.raw(shape, 'shell'), str(bool(phantom)).lower(), t.text(shape, 'props')])
    return result, [p.report, t.report]


if __name__ == '__main__':
    a = argparse.ArgumentParser()
    a.add_argument('game_root', type=Path)
    a.add_argument('native_report', type=Path)
    a.add_argument('--output', required=True, type=Path)
    args = a.parse_args()
    private = (Path(__file__).resolve().parent.parent/'local').resolve()
    if not args.output.resolve().is_relative_to(private):
        raise SystemExit('Evidence must stay under local/')
    expected, sources = rows(args.game_root)
    actual = [line.split('\t') for line in args.native_report.read_text().splitlines()]
    reference = collections.Counter(map(tuple, expected))
    native = collections.Counter(map(tuple, actual))
    missing = reference - native
    extra = native - reference
    report = dict(exact=not missing and not extra, counts=dict(collections.Counter(r[0] for r in expected)),
                  missing=list(missing.items()), extra=list(extra.items()),
                  inputs=[{k: s[k] for k in ('path', 'bytes', 'sha256')} for s in sources],
                  native_sha256=hashlib.sha256(args.native_report.read_bytes()).hexdigest())
    with args.output.open('x', encoding='utf-8') as f:
        json.dump(report, f, indent=2)
    print(json.dumps({k: report[k] for k in ('exact', 'counts')}))
    if not report['exact']:
        raise SystemExit('Native entity descriptors differ from independent original-byte resolution')
