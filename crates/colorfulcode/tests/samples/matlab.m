function [mu, sigma] = stats(data, varargin)
%STATS Mean and standard deviation of a vector.
%   [MU, SIGMA] = STATS(DATA) ignores NaN values.

%{
  Block comment: the function also accepts
  an optional 'Weights' parameter.
%}

if nargin < 1 || isempty(data)
    error('stats:empty', 'Input must not be empty');
end

data = data(~isnan(data));
w = ones(size(data));
for k = 1:2:numel(varargin)
    switch lower(varargin{k})
        case 'weights'
            w = varargin{k+1}';
        otherwise
            warning("Unknown option ""%s""", varargin{k});
    end
end

mu = sum(w .* data) / sum(w);
sigma = sqrt(sum(w .* (data - mu).^2) / (sum(w) - 1));
A = [1 2; 3 4]';
B = A.' * inv(A) + 1e-3;
s.name = 'it''s';
fprintf('mu = %.3f, sigma = %.3f\n', mu, sigma);  % trailing comment
x = linspace(0, 2*pi, 100); ...
    plot(x, sin(x));
end

function out = helper(x)
    out = x * pi;
end
