// Kubernetes deployment generator
local k = import 'k.libsonnet';
local defaults = importstr "defaults.txt";

local container(name, image, port=80) = {
  name: name,
  image: image,
  ports: [{ containerPort: port }],
};

/* Build a deployment */
local deployment(name, replicas=1) = {
  apiVersion: 'apps/v1',
  kind: "Deployment",
  metadata: { name: name, labels+: { app: name } },
  spec: {
    replicas: replicas,
    template: {
      spec: {
        containers: [container(name, 'nginx:1.25')],
      },
    },
  },
  hidden:: 'not rendered',
  forced::: 'always rendered',
  ratio: 0.75 * 2,
  enabled: !false && true,
  nothing: null,
};

{
  web: deployment('web', 3) + {
    spec+: { replicas: $.config.replicas },
  },
  config:: { replicas: 5 },
  names: [d.metadata.name for d in std.objectValues(self) if std.isObject(d)],
  text: |||
    Hello, world.
    This is a text block.
  |||,
  verbatim: @'C:\path\''s',
  conditional: if std.length(defaults) > 0 then 'yes' else 'no',
  assert self.config.replicas > 0 : 'replicas must be positive',
  # hash comment
  escaped: "tab\tnewline\n",
}
